//! Background scheduler: periodically claims due monitors and dispatches
//! checks through the same `monitoring_service::run_check` path used by the
//! manual "Check now" endpoint (one code path for retries, SSRF, incidents).
//!
//! Claiming is atomic in SQL (`monitor_repository::claim_due`): each claimed
//! monitor's `next_check_at` advances to `now + interval`, so a monitor is
//! never claimed twice concurrently and an overdue monitor after a restart
//! is checked exactly once (no catch-up burst). Actual concurrency is
//! bounded by the semaphore inside `run_check`.

use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;

use crate::repositories::monitor_repository;
use crate::services::monitoring_service::{self, CheckError, CheckTrigger};
use crate::state::AppState;

/// How often the scheduler scans for due monitors.
pub const TICK_INTERVAL: Duration = Duration::from_secs(1);

/// One scan: claim up to `max_concurrent_checks` due monitors and spawn a
/// check task for each. Returns the spawned tasks so callers (and tests) can
/// await completion. Claim errors are logged and treated as an empty scan so
/// one bad tick cannot kill the loop.
pub async fn run_due_once(state: &AppState) -> Vec<JoinHandle<()>> {
    let now = Utc::now();
    let max_results = state.config.max_concurrent_checks as i64;
    let claimed = match monitor_repository::claim_due(&state.pool, now, max_results).await {
        Ok(claimed) => claimed,
        Err(err) => {
            tracing::error!(error = %err, "failed to claim due monitors");
            return Vec::new();
        }
    };

    let mut handles = Vec::with_capacity(claimed.len());
    for monitor in claimed {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            match monitoring_service::run_check(&state, monitor.id, CheckTrigger::Scheduled).await {
                Ok(outcome) => {
                    tracing::info!(
                        monitor_id = %monitor.id,
                        success = outcome.result.success,
                        status = outcome.monitor.current_status.as_str(),
                        "scheduled check finished"
                    );
                }
                // Paused/removed between claim and dispatch, or a manual
                // check is already running: nothing to do.
                Err(CheckError::AlreadyRunning | CheckError::Paused | CheckError::NotFound) => {
                    tracing::debug!(monitor_id = %monitor.id, "scheduled check skipped");
                }
                Err(err) => {
                    tracing::warn!(monitor_id = %monitor.id, error = %err, "scheduled check failed");
                }
            }
        }));
    }
    handles
}

/// Spawn the scheduler loop. The loop stops when the shared shutdown token
/// is cancelled (SIGINT/SIGTERM path in `main`).
pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        // After a slow tick, skip missed beats instead of bursting.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(
            tick_ms = TICK_INTERVAL.as_millis() as u64,
            "scheduler started"
        );
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => {
                    tracing::info!("scheduler stopped");
                    break;
                }
                _ = ticker.tick() => {
                    let handles = run_due_once(&state).await;
                    if !handles.is_empty() {
                        tracing::debug!(dispatched = handles.len(), "scheduled checks dispatched");
                    }
                }
            }
        }
    })
}
