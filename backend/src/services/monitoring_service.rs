//! The single check-execution path shared by scheduled and manual checks.
//!
//! Flow per check:
//! validate URL → acquire concurrency permit → attempt loop (timeout, retry
//! with backoff, `Retry-After`) → persist `CheckResult` + monitor counters +
//! incident transition in **one transaction**.
//!
//! Response time is measured as wall-clock time from request start until the
//! response *headers* arrive (`reqwest::Response` resolves); the body is never
//! downloaded. Transport failures have no response time (never coerced to 0).

use std::time::{Duration, Instant};

use chrono::Utc;
use uuid::Uuid;

use crate::config::{MAX_TIMEOUT_MS, MIN_TIMEOUT_MS};
use crate::error::AppError;
use crate::models::check_result::{CheckErrorType, CheckResult};
use crate::models::monitor::{Monitor, MonitorStatus};
use crate::repositories::{check_repository, monitor_repository};
use crate::services::incident_service;
use crate::services::retry::RetryPolicy;
use crate::state::AppState;
use crate::validation::{
    chain_contains, validate_monitor_url, BlockedTarget, SsrfPolicy, TooManyRedirects,
};

/// Longest error message persisted (sanitized; never contains headers/bodies).
const MAX_ERROR_MESSAGE: usize = 500;

/// Why a check ran — carried into logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckTrigger {
    Scheduled,
    Manual,
}

impl CheckTrigger {
    fn as_str(&self) -> &'static str {
        match self {
            CheckTrigger::Scheduled => "scheduled",
            CheckTrigger::Manual => "manual",
        }
    }
}

/// Everything the caller (API response or scheduler log) needs after a check.
#[derive(Debug)]
pub struct CheckOutcome {
    /// Monitor row after the outcome was applied.
    pub monitor: Monitor,
    pub result: CheckResult,
    pub incident_opened: bool,
    pub incident_resolved: bool,
}

/// Failures that prevent a check from running at all (HTTP-mapped).
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("monitor not found")]
    NotFound,
    #[error("a check for this monitor is already running")]
    AlreadyRunning,
    #[error("monitor is paused; resume it before checking")]
    Paused,
    #[error("internal error: {0}")]
    Internal(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<CheckError> for AppError {
    fn from(err: CheckError) -> Self {
        match err {
            CheckError::NotFound => AppError::NotFound("monitor not found".to_string()),
            CheckError::AlreadyRunning | CheckError::Paused => AppError::Conflict(err.to_string()),
            CheckError::Internal(message) => AppError::Internal(message),
            CheckError::Database(err) => AppError::Database(err),
        }
    }
}

/// Run one check for `monitor_id` end to end.
///
/// Manual and scheduled checks share this path so behaviour, concurrency
/// limits and persistence can never drift apart.
pub async fn run_check(
    state: &AppState,
    monitor_id: Uuid,
    trigger: CheckTrigger,
) -> Result<CheckOutcome, CheckError> {
    let monitor = monitor_repository::find_by_id(&state.pool, monitor_id)
        .await?
        .ok_or(CheckError::NotFound)?;

    if !monitor.enabled || monitor.current_status == MonitorStatus::Paused {
        return Err(CheckError::Paused);
    }

    // Overlap guard: one in-flight check per monitor, whichever trigger wins.
    if !state.in_flight.try_begin(monitor_id).await {
        return Err(CheckError::AlreadyRunning);
    }
    let outcome = execute_and_persist(state, monitor, trigger).await;
    state.in_flight.end(monitor_id).await;
    outcome
}

async fn execute_and_persist(
    state: &AppState,
    monitor: Monitor,
    trigger: CheckTrigger,
) -> Result<CheckOutcome, CheckError> {
    // Bound all outbound work globally; the permit is released when the
    // check finishes (including retry sleeps).
    let _permit = state
        .semaphore
        .acquire()
        .await
        .map_err(|_| CheckError::Internal("concurrency limiter closed".to_string()))?;

    let retry = RetryPolicy::for_monitor(&monitor, &state.config);
    let timeout =
        Duration::from_millis(monitor.timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS) as u64);
    let checked_at = Utc::now();
    let started = Instant::now();

    tracing::info!(
        monitor_id = %monitor.id,
        trigger = trigger.as_str(),
        "check started"
    );

    let mut attempt: i64 = 0;
    let final_outcome = loop {
        attempt += 1;
        let outcome = execute_attempt(state, &monitor, timeout).await;
        match &outcome {
            AttemptOutcome::Success { .. } => break outcome,
            AttemptOutcome::Failure {
                error_type,
                http_status,
                retry_after,
                ..
            } => {
                if !retry.should_retry(*error_type, *http_status, attempt) {
                    break outcome;
                }
                let delay = retry_after
                    .as_deref()
                    .and_then(|header| {
                        RetryPolicy::retry_after_delay(
                            Some(header),
                            *http_status,
                            retry.max_backoff_millis(),
                        )
                    })
                    .unwrap_or_else(|| retry.backoff(attempt));
                tracing::debug!(
                    monitor_id = %monitor.id,
                    attempt,
                    delay_ms = delay.as_millis() as u64,
                    error_type = error_type.as_str(),
                    "retrying after transient failure"
                );
                tokio::time::sleep(delay).await;
            }
        }
    };

    let elapsed_ms = started.elapsed().as_millis() as i64;
    let completed_at = Utc::now();

    let transition = incident_service::plan_transition(
        &monitor,
        matches!(final_outcome, AttemptOutcome::Success { .. }),
    );
    let (result, failure_reason) =
        build_check_result(&monitor, final_outcome, checked_at, completed_at, attempt);

    // Scheduled checks keep the interval claimed at dispatch time; manual
    // checks re-arm the schedule so the monitor joins/refreshes its cadence.
    let next_check_at = match trigger {
        CheckTrigger::Scheduled => monitor.next_check_at,
        CheckTrigger::Manual => {
            Some(completed_at + chrono::Duration::seconds(monitor.check_interval_seconds))
        }
    };

    let mut tx = state.pool.begin().await?;
    check_repository::insert(&mut *tx, &result).await?;
    let transition_outcome = incident_service::persist_check_outcome(
        &mut tx,
        &monitor,
        transition,
        failure_reason.as_deref(),
        checked_at,
        next_check_at,
    )
    .await?;
    tx.commit().await?;

    tracing::info!(
        monitor_id = %monitor.id,
        trigger = trigger.as_str(),
        attempt,
        elapsed_ms,
        success = result.success,
        error_type = result.error_type.map(|e| e.as_str()).unwrap_or("none"),
        status = transition_outcome.monitor.current_status.as_str(),
        "check completed"
    );

    Ok(CheckOutcome {
        monitor: transition_outcome.monitor,
        incident_opened: transition_outcome.incident_opened.is_some(),
        incident_resolved: transition_outcome.incident_resolved.is_some(),
        result,
    })
}

/// Outcome of a single HTTP attempt (before retries).
enum AttemptOutcome {
    Success {
        http_status: i64,
        response_time_ms: i64,
    },
    Failure {
        error_type: CheckErrorType,
        http_status: Option<i64>,
        response_time_ms: Option<i64>,
        message: String,
        retry_after: Option<String>,
    },
}

async fn execute_attempt(state: &AppState, monitor: &Monitor, timeout: Duration) -> AttemptOutcome {
    // Re-validate in case policy tightened since the URL was stored.
    if let Err(err) = validate_monitor_url(&monitor.url, &SsrfPolicy::from_config(&state.config)) {
        return AttemptOutcome::Failure {
            error_type: CheckErrorType::BlockedTarget,
            http_status: None,
            response_time_ms: None,
            message: sanitize_message(&format!("monitor URL rejected: {err}")),
            retry_after: None,
        };
    }

    let request = state.http_client.get(monitor.url.clone());
    let started = Instant::now();
    let send = tokio::time::timeout(timeout, request.send()).await;
    let response_time_ms = started.elapsed().as_millis() as i64;

    match send {
        Err(_) => AttemptOutcome::Failure {
            error_type: CheckErrorType::Timeout,
            http_status: None,
            response_time_ms: None,
            message: sanitize_message(&format!("timed out after {} ms", timeout.as_millis())),
            retry_after: None,
        },
        Ok(Err(err)) => {
            let (error_type, message) = classify_reqwest_error(&err);
            let http_status = err.status().map(|status| i64::from(status.as_u16()));
            AttemptOutcome::Failure {
                error_type,
                http_status,
                // Only a received response has a meaningful response time.
                response_time_ms: http_status.map(|_| response_time_ms),
                message: sanitize_message(&message),
                retry_after: None,
            }
        }
        Ok(Ok(response)) => {
            let http_status = i64::from(response.status().as_u16());
            let expected =
                (monitor.expected_status_min..=monitor.expected_status_max).contains(&http_status);
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);
            if expected {
                AttemptOutcome::Success {
                    http_status,
                    response_time_ms,
                }
            } else {
                AttemptOutcome::Failure {
                    error_type: CheckErrorType::HttpStatusMismatch,
                    http_status: Some(http_status),
                    response_time_ms: Some(response_time_ms),
                    message: sanitize_message(&format!(
                        "unexpected HTTP status {http_status} (expected {}-{})",
                        monitor.expected_status_min, monitor.expected_status_max
                    )),
                    retry_after,
                }
            }
        }
    }
}

fn build_check_result(
    monitor: &Monitor,
    outcome: AttemptOutcome,
    checked_at: chrono::DateTime<Utc>,
    completed_at: chrono::DateTime<Utc>,
    attempt_count: i64,
) -> (CheckResult, Option<String>) {
    let id = Uuid::new_v4();
    match outcome {
        AttemptOutcome::Success {
            http_status,
            response_time_ms,
        } => (
            CheckResult {
                id,
                monitor_id: monitor.id,
                checked_at,
                completed_at: Some(completed_at),
                http_status_code: Some(http_status),
                response_time_ms: Some(response_time_ms),
                success: true,
                error_type: None,
                error_message: None,
                attempt_count,
                expected_status_match: true,
            },
            None,
        ),
        AttemptOutcome::Failure {
            error_type,
            http_status,
            response_time_ms,
            message,
            ..
        } => {
            let failure_reason = format!("{}: {}", error_type.as_str(), message);
            (
                CheckResult {
                    id,
                    monitor_id: monitor.id,
                    checked_at,
                    completed_at: Some(completed_at),
                    http_status_code: http_status,
                    response_time_ms,
                    success: false,
                    error_type: Some(error_type),
                    error_message: Some(message.clone()),
                    attempt_count,
                    expected_status_match: false,
                },
                Some(failure_reason),
            )
        }
    }
}

/// Map a reqwest failure onto the check error taxonomy.
///
/// Marker errors planted by the SSRF layers are checked first so a blocked
/// destination is distinguishable from an ordinary DNS/redirect failure.
/// TLS is detected by a keyword scan of the error chain (rustls is not a
/// direct dependency, so the concrete type cannot be downcast).
fn classify_reqwest_error(err: &reqwest::Error) -> (CheckErrorType, String) {
    let error_type = if chain_contains::<BlockedTarget>(err) {
        CheckErrorType::BlockedTarget
    } else if chain_contains::<TooManyRedirects>(err) {
        CheckErrorType::TooManyRedirects
    } else if err.is_timeout() {
        CheckErrorType::Timeout
    } else if err.is_dns() {
        CheckErrorType::Dns
    } else if err.is_redirect() {
        CheckErrorType::TooManyRedirects
    } else if err.is_connect() {
        if chain_looks_like_tls(err) {
            CheckErrorType::Tls
        } else {
            CheckErrorType::Connection
        }
    } else if err.is_request() || err.is_upgrade() {
        CheckErrorType::Connection
    } else {
        // Body/decode failures and anything unclassified: the request was
        // sent but the exchange broke afterwards.
        CheckErrorType::Internal
    };
    (error_type, describe_error(err))
}

/// Compact rendering of an error chain (top-level kind + up to 3 sources).
fn describe_error(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();
    let mut depth = 0;
    while let Some(source) = current {
        if depth >= 3 {
            break;
        }
        parts.push(source.to_string());
        current = source.source();
        depth += 1;
    }
    parts.join(": ")
}

fn chain_looks_like_tls(err: &reqwest::Error) -> bool {
    const KEYWORDS: [&str; 6] = ["certificate", "handshake", "tls", "ssl", "rustls", "pem"];
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(err) = current {
        let text = err.to_string().to_ascii_lowercase();
        if KEYWORDS.iter().any(|kw| text.contains(kw)) {
            return true;
        }
        current = err.source();
    }
    false
}

fn sanitize_message(message: &str) -> String {
    let message = message.trim();
    if message.chars().count() <= MAX_ERROR_MESSAGE {
        return message.to_string();
    }
    message.chars().take(MAX_ERROR_MESSAGE).collect::<String>() + "…"
}
