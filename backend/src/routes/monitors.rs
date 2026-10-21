//! Monitor CRUD and control endpoints (create/list/get/update/delete +
//! check-now/pause/resume). All handlers delegate to repositories/services —
//! no SQL or check logic lives here.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::AdminSession;
use crate::config::{
    MAX_CHECK_INTERVAL_SECS, MAX_RETRY_ATTEMPTS, MAX_TIMEOUT_MS, MIN_CHECK_INTERVAL_SECS,
    MIN_RETRY_ATTEMPTS, MIN_TIMEOUT_MS,
};
use crate::error::{AppError, FieldError};
use crate::models::check_result::CheckResult;
use crate::models::monitor::{
    CreateMonitorRequest, ListMonitorsQuery, Monitor, MonitorSummary, UpdateMonitorRequest,
};
use crate::models::ReportWindow;
use crate::repositories::monitor_repository;
use crate::services::lifecycle_service;
use crate::services::monitoring_service::{self, CheckTrigger};
use crate::services::settings_service::AppSettings;
use crate::state::AppState;
use crate::validation::{validate_monitor_url, SsrfPolicy};

/// Pagination metadata returned with list endpoints.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct Pagination {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub total_pages: i64,
}

impl Pagination {
    pub fn new(page: i64, page_size: i64, total: i64) -> Self {
        let total_pages = if total == 0 {
            0
        } else {
            (total + page_size - 1) / page_size
        };
        Self {
            page,
            page_size,
            total,
            total_pages,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedMonitors {
    pub data: Vec<MonitorSummary>,
    pub pagination: Pagination,
}

/// Result of running a check immediately.
#[derive(Debug, Serialize, ToSchema)]
pub struct CheckResponse {
    pub monitor: Monitor,
    pub check: CheckResult,
    pub incident_opened: bool,
    pub incident_resolved: bool,
}

/// List monitors with search/filter/sort/pagination.
#[utoipa::path(
    get,
    path = "/api/v1/monitors",
    tag = "monitors",
    params(ListMonitorsQuery),
    responses(
        (status = 200, description = "Paginated monitor list", body = PaginatedMonitors),
        (status = 422, description = "Invalid query parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListMonitorsQuery>,
) -> Result<Json<PaginatedMonitors>, AppError> {
    let window = ReportWindow::from_query(
        query.window.as_deref(),
        query.from.as_deref(),
        query.to.as_deref(),
    )?;
    validate_status_filter(query.status.as_deref())?;

    let (monitors, total) = monitor_repository::list(&state.pool, &query, window.start).await?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let data = summarize_many(&state, monitors, window).await?;
    Ok(Json(PaginatedMonitors {
        data,
        pagination: Pagination::new(page, page_size, total),
    }))
}

/// Get one monitor with its latest check and windowed uptime.
#[utoipa::path(
    get,
    path = "/api/v1/monitors/{id}",
    tag = "monitors",
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 200, description = "Monitor details", body = MonitorSummary),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MonitorSummary>, AppError> {
    let monitor = find_monitor(&state, id).await?;
    let window = ReportWindow::from_query(Some("24h"), None, None)?;
    let summary = summarize(&state, monitor, window).await?;
    Ok(Json(summary))
}

/// Create a monitor. The first scheduled check runs on the next scheduler
/// tick (`next_check_at` is set to "now").
#[utoipa::path(
    post,
    path = "/api/v1/monitors",
    tag = "monitors",
    security(("session_cookie" = [])),
    request_body = CreateMonitorRequest,
    responses(
        (status = 201, description = "Monitor created", body = MonitorSummary),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 422, description = "Validation failed", body = crate::error::ErrorResponse),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    _session: AdminSession,
    Json(request): Json<CreateMonitorRequest>,
) -> Result<(StatusCode, Json<MonitorSummary>), AppError> {
    let defaults = AppSettings::load(&state.pool, &state.config)
        .await
        .map_err(AppError::Database)?;
    let fields = validate_config(
        &request.url,
        ConfigOverrides {
            check_interval_seconds: request.check_interval_seconds,
            timeout_ms: request.timeout_ms,
            expected_status_min: request.expected_status_min,
            expected_status_max: request.expected_status_max,
            consecutive_failures_threshold: request.consecutive_failures_threshold,
            consecutive_successes_threshold: request.consecutive_successes_threshold,
            retry_max_attempts: request.retry_max_attempts,
        },
        None,
        &state,
        &defaults,
    )?;
    let name = normalize_name(request.name.as_deref(), &request.url)?;
    let alert_webhook_url = normalize_alert_webhook(request.alert_webhook_url.as_deref(), &state)?;
    let alert_email = normalize_alert_email(request.alert_email.as_deref())?;

    let now = Utc::now();
    let monitor = Monitor {
        id: Uuid::new_v4(),
        name,
        url: fields.url,
        enabled: true,
        check_interval_seconds: fields.check_interval_seconds,
        timeout_ms: fields.timeout_ms,
        expected_status_min: fields.expected_status_min,
        expected_status_max: fields.expected_status_max,
        consecutive_failures_threshold: fields.consecutive_failures_threshold,
        consecutive_successes_threshold: fields.consecutive_successes_threshold,
        retry_max_attempts: fields.retry_max_attempts,
        alert_webhook_url,
        alert_email,
        current_status: crate::models::monitor::MonitorStatus::Unknown,
        consecutive_failures: 0,
        consecutive_successes: 0,
        created_at: now,
        updated_at: now,
        last_checked_at: None,
        // Due immediately: the scheduler picks it up on its next tick.
        next_check_at: Some(now),
    };

    monitor_repository::insert(&state.pool, &monitor).await?;
    tracing::info!(monitor_id = %monitor.id, "monitor created");
    let window = ReportWindow::from_query(Some("24h"), None, None)?;
    let summary = summarize(&state, monitor, window).await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

/// Update an existing monitor (partial: only provided fields change).
#[utoipa::path(
    put,
    path = "/api/v1/monitors/{id}",
    tag = "monitors",
    security(("session_cookie" = [])),
    params(("id" = Uuid, Path, description = "Monitor id")),
    request_body = UpdateMonitorRequest,
    responses(
        (status = 200, description = "Monitor updated", body = MonitorSummary),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Validation failed", body = crate::error::ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AdminSession,
    Json(request): Json<UpdateMonitorRequest>,
) -> Result<Json<MonitorSummary>, AppError> {
    let mut monitor = find_monitor(&state, id).await?;

    let defaults = AppSettings::load(&state.pool, &state.config)
        .await
        .map_err(AppError::Database)?;
    let fields = validate_config(
        request.url.as_deref().unwrap_or(&monitor.url),
        ConfigOverrides {
            check_interval_seconds: request.check_interval_seconds,
            timeout_ms: request.timeout_ms,
            expected_status_min: request.expected_status_min,
            expected_status_max: request.expected_status_max,
            consecutive_failures_threshold: request.consecutive_failures_threshold,
            consecutive_successes_threshold: request.consecutive_successes_threshold,
            retry_max_attempts: request.retry_max_attempts,
        },
        Some(&monitor),
        &state,
        &defaults,
    )?;
    if let Some(name) = request.name.as_deref() {
        monitor.name = normalize_name(Some(name), &monitor.url)?;
    }
    monitor.url = fields.url;
    monitor.check_interval_seconds = fields.check_interval_seconds;
    monitor.timeout_ms = fields.timeout_ms;
    monitor.expected_status_min = fields.expected_status_min;
    monitor.expected_status_max = fields.expected_status_max;
    monitor.consecutive_failures_threshold = fields.consecutive_failures_threshold;
    monitor.consecutive_successes_threshold = fields.consecutive_successes_threshold;
    monitor.retry_max_attempts = fields.retry_max_attempts;
    if let Some(raw) = request.alert_webhook_url.as_deref() {
        monitor.alert_webhook_url = normalize_alert_webhook(Some(raw), &state)?;
    }
    if let Some(raw) = request.alert_email.as_deref() {
        monitor.alert_email = normalize_alert_email(Some(raw))?;
    }
    monitor.updated_at = Utc::now();

    if !monitor_repository::update(&state.pool, &monitor).await? {
        return Err(AppError::NotFound("monitor not found".to_string()));
    }
    tracing::info!(monitor_id = %monitor.id, "monitor updated");
    let window = ReportWindow::from_query(Some("24h"), None, None)?;
    let summary = summarize(&state, monitor, window).await?;
    Ok(Json(summary))
}

/// Delete a monitor (cascades to its checks and incidents).
#[utoipa::path(
    delete,
    path = "/api/v1/monitors/{id}",
    tag = "monitors",
    security(("session_cookie" = [])),
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AdminSession,
) -> Result<StatusCode, AppError> {
    if !monitor_repository::delete(&state.pool, id).await? {
        return Err(AppError::NotFound("monitor not found".to_string()));
    }
    tracing::info!(monitor_id = %id, "monitor deleted");
    Ok(StatusCode::NO_CONTENT)
}

/// Run one check immediately ("Check now"). Shares the full check pipeline
/// (validation, concurrency limit, retries, persistence) with the scheduler.
#[utoipa::path(
    post,
    path = "/api/v1/monitors/{id}/check",
    tag = "monitors",
    security(("session_cookie" = [])),
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 200, description = "Check completed", body = CheckResponse),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Monitor paused or a check is already running", body = crate::error::ErrorResponse),
    )
)]
pub async fn check_now(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AdminSession,
) -> Result<Json<CheckResponse>, AppError> {
    let outcome = monitoring_service::run_check(&state, id, CheckTrigger::Manual).await?;
    Ok(Json(CheckResponse {
        monitor: outcome.monitor,
        check: outcome.result,
        incident_opened: outcome.incident_opened,
        incident_resolved: outcome.incident_resolved,
    }))
}

/// Pause a monitor: no scheduled or manual checks until resumed, and it is
/// never reported as operational.
#[utoipa::path(
    post,
    path = "/api/v1/monitors/{id}/pause",
    tag = "monitors",
    security(("session_cookie" = [])),
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 200, description = "Monitor paused", body = MonitorSummary),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
    )
)]
pub async fn pause(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AdminSession,
) -> Result<Json<MonitorSummary>, AppError> {
    let outcome = lifecycle_service::pause_monitor(&state, id).await?;
    tracing::info!(
        monitor_id = %id,
        resolved_incident_id = ?outcome.resolved_incident_id,
        "monitor paused"
    );
    let window = ReportWindow::from_query(Some("24h"), None, None)?;
    let summary = summarize(&state, outcome.monitor, window).await?;
    Ok(Json(summary))
}

/// Resume a paused monitor. Status resets to `unknown` (a paused monitor must
/// never be reported as up) and the first check is due immediately.
#[utoipa::path(
    post,
    path = "/api/v1/monitors/{id}/resume",
    tag = "monitors",
    security(("session_cookie" = [])),
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 200, description = "Monitor resumed", body = MonitorSummary),
        (status = 401, description = "Authentication required", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
    )
)]
pub async fn resume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    _session: AdminSession,
) -> Result<Json<MonitorSummary>, AppError> {
    let outcome = lifecycle_service::resume_monitor(&state, id).await?;
    tracing::info!(
        monitor_id = %id,
        closed_interval = outcome.closed_interval,
        "monitor resumed"
    );
    let window = ReportWindow::from_query(Some("24h"), None, None)?;
    let summary = summarize(&state, outcome.monitor, window).await?;
    Ok(Json(summary))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn find_monitor(state: &AppState, id: Uuid) -> Result<Monitor, AppError> {
    monitor_repository::find_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))
}

/// Fields of a create/update request after defaults + validation are applied.
struct ValidatedConfig {
    url: String,
    check_interval_seconds: i64,
    timeout_ms: i64,
    expected_status_min: i64,
    expected_status_max: i64,
    consecutive_failures_threshold: i64,
    consecutive_successes_threshold: i64,
    retry_max_attempts: i64,
}

/// Optional numeric overrides from a request body. `None` means "keep the
/// current value (update) or the process default (create)".
#[derive(Debug, Default, Clone, Copy)]
struct ConfigOverrides {
    check_interval_seconds: Option<i64>,
    timeout_ms: Option<i64>,
    expected_status_min: Option<i64>,
    expected_status_max: Option<i64>,
    consecutive_failures_threshold: Option<i64>,
    consecutive_successes_threshold: Option<i64>,
    retry_max_attempts: Option<i64>,
}

fn validate_status_filter(status: Option<&str>) -> Result<(), AppError> {
    if let Some(status) = status {
        if crate::models::monitor::MonitorStatus::parse(status).is_none() {
            return Err(AppError::validation(
                "status",
                "must be one of unknown, up, down, paused",
            ));
        }
    }
    Ok(())
}

/// Validate the merged configuration: URL against the SSRF policy, numeric
/// fields against their documented bounds. `existing` supplies the baseline
/// for fields the request omits; `defaults` supplies new-monitor fallbacks.
fn validate_config(
    url: &str,
    overrides: ConfigOverrides,
    existing: Option<&Monitor>,
    state: &AppState,
    defaults: &AppSettings,
) -> Result<ValidatedConfig, AppError> {
    let url = validate_monitor_url(url, &SsrfPolicy::from_config(&state.config))
        .map_err(|err| AppError::validation("url", err.to_string()))?;

    let current = |field: fn(&Monitor) -> i64, default: i64| existing.map(field).unwrap_or(default);
    let mut errors: Vec<FieldError> = Vec::new();

    let check_interval_seconds = effective(
        &mut errors,
        "check_interval_seconds",
        overrides.check_interval_seconds,
        current(
            |m| m.check_interval_seconds,
            defaults.default_check_interval_seconds,
        ),
        MIN_CHECK_INTERVAL_SECS,
        MAX_CHECK_INTERVAL_SECS,
    );
    let timeout_ms = effective(
        &mut errors,
        "timeout_ms",
        overrides.timeout_ms,
        current(|m| m.timeout_ms, defaults.default_timeout_ms),
        MIN_TIMEOUT_MS,
        MAX_TIMEOUT_MS,
    );
    let retry_max_attempts = effective(
        &mut errors,
        "retry_max_attempts",
        overrides.retry_max_attempts,
        current(
            |m| m.retry_max_attempts,
            defaults.default_retry_max_attempts,
        ),
        MIN_RETRY_ATTEMPTS,
        MAX_RETRY_ATTEMPTS,
    );
    let expected_status_min = effective(
        &mut errors,
        "expected_status_min",
        overrides.expected_status_min,
        current(
            |m| m.expected_status_min,
            defaults.default_expected_status_min,
        ),
        100,
        599,
    );
    let expected_status_max = effective(
        &mut errors,
        "expected_status_max",
        overrides.expected_status_max,
        current(
            |m| m.expected_status_max,
            defaults.default_expected_status_max,
        ),
        100,
        599,
    );
    if expected_status_min > expected_status_max {
        errors.push(FieldError {
            field: "expected_status_max".to_string(),
            message: "must be >= expected_status_min".to_string(),
        });
    }
    let consecutive_failures_threshold = effective(
        &mut errors,
        "consecutive_failures_threshold",
        overrides.consecutive_failures_threshold,
        current(|m| m.consecutive_failures_threshold, 1),
        1,
        100,
    );
    let consecutive_successes_threshold = effective(
        &mut errors,
        "consecutive_successes_threshold",
        overrides.consecutive_successes_threshold,
        current(|m| m.consecutive_successes_threshold, 1),
        1,
        100,
    );

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    Ok(ValidatedConfig {
        url: url.to_string(),
        check_interval_seconds,
        timeout_ms,
        expected_status_min,
        expected_status_max,
        consecutive_failures_threshold,
        consecutive_successes_threshold,
        retry_max_attempts,
    })
}

/// `override` (if set) else `current`, range-checked against `[min, max]`.
fn effective(
    errors: &mut Vec<FieldError>,
    field: &str,
    override_value: Option<i64>,
    current: i64,
    min: i64,
    max: i64,
) -> i64 {
    let value = override_value.unwrap_or(current);
    if value < min || value > max {
        errors.push(FieldError {
            field: field.to_string(),
            message: format!("must be between {min} and {max}"),
        });
    }
    value
}

/// Validate and normalize an optional alert webhook. An empty string clears
/// the destination; the same SSRF policy as monitor targets applies.
fn normalize_alert_webhook(
    raw: Option<&str>,
    state: &AppState,
) -> Result<Option<String>, AppError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = validate_monitor_url(value, &SsrfPolicy::from_config(&state.config))
        .map_err(|err| AppError::validation("alert_webhook_url", err.to_string()))?;
    Ok(Some(parsed.to_string()))
}

/// Basic email validation: one non-empty local/domain part, no whitespace, and
/// a dotted domain. The SMTP server remains the final authority.
fn normalize_alert_email(raw: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > 254 || value.chars().any(char::is_whitespace) {
        return Err(AppError::validation(
            "alert_email",
            "must be a valid email address",
        ));
    }
    let mut parts = value.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty()
        || domain.is_empty()
        || parts.next().is_some()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(AppError::validation(
            "alert_email",
            "must be a valid email address",
        ));
    }
    Ok(Some(value.to_string()))
}

fn normalize_name(name: Option<&str>, url: &str) -> Result<String, AppError> {
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_string(),
        None => url_host(url).unwrap_or_else(|| "monitor".to_string()),
    };
    if name.is_empty() {
        return Err(AppError::validation("name", "must not be empty"));
    }
    if name.chars().count() > 100 {
        return Err(AppError::validation(
            "name",
            "must be at most 100 characters",
        ));
    }
    Ok(name)
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(ToString::to_string))
}

async fn summarize(
    state: &AppState,
    monitor: Monitor,
    window: ReportWindow,
) -> Result<MonitorSummary, AppError> {
    let mut summaries = summarize_many(state, vec![monitor], window).await?;
    summaries
        .pop()
        .ok_or_else(|| AppError::Internal("summary missing".to_string()))
}

async fn summarize_many(
    state: &AppState,
    monitors: Vec<Monitor>,
    window: ReportWindow,
) -> Result<Vec<MonitorSummary>, AppError> {
    if monitors.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<Uuid> = monitors.iter().map(|m| m.id).collect();
    let stats =
        monitor_repository::window_stats(&state.pool, &ids, window.start, window.end).await?;
    let latest = monitor_repository::latest_checks(&state.pool, &ids).await?;

    Ok(monitors
        .into_iter()
        .map(|monitor| {
            let stat = stats.get(&monitor.id);
            let last = latest.get(&monitor.id);
            let checks_in_window = stat.map(|s| s.checks).unwrap_or(0);
            let uptime_percent = if checks_in_window == 0 {
                None
            } else {
                Some(
                    stat.map(|s| s.successes as f64 / s.checks as f64 * 100.0)
                        .unwrap_or(0.0),
                )
            };
            let degraded = monitor.current_status == crate::models::monitor::MonitorStatus::Up
                && monitor.consecutive_failures > 0;
            MonitorSummary {
                latest_http_status: last.and_then(|l| l.http_status_code),
                latest_response_time_ms: last.and_then(|l| l.response_time_ms),
                latest_success: last.map(|l| l.success),
                uptime_percent,
                checks_in_window,
                degraded,
                monitor,
            }
        })
        .collect())
}
