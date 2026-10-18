//! Status transitions and incident lifecycle driven by check outcomes.
//!
//! State machine:
//!
//! ```text
//! unknown ──fail(≥failure_threshold)──► down   [opens incident]
//! up      ──fail(≥failure_threshold)──► down   [opens incident if none open]
//! down    ──success(≥success_threshold)──► up  [resolves open incident]
//! any     ──pause──────────────────────► paused [no checks until resume]
//! ```
//!
//! Every transition runs inside the same transaction as the check result
//! insert, so a monitor can never show a status its history doesn't support.
//! The partial unique index `idx_incidents_one_open` guarantees at most one
//! open incident per monitor even under races.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use crate::models::incident::Incident;
use crate::models::monitor::{Monitor, MonitorStatus};
use crate::repositories::{incident_repository, monitor_repository};

/// Status + counters implied by one completed check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusTransition {
    pub status: MonitorStatus,
    pub consecutive_failures: i64,
    pub consecutive_successes: i64,
}

/// Pure decision function: given the monitor row before the check and the
/// check outcome, what status and counters apply afterwards?
pub fn plan_transition(monitor: &Monitor, success: bool) -> StatusTransition {
    if success {
        let successes = monitor.consecutive_successes + 1;
        let status = if successes >= monitor.consecutive_successes_threshold.max(1) {
            MonitorStatus::Up
        } else {
            // Not enough confirmations yet: keep the previous operational
            // truth (down stays down, unknown stays unknown).
            monitor.current_status
        };
        StatusTransition {
            status,
            consecutive_failures: 0,
            consecutive_successes: successes,
        }
    } else {
        let failures = monitor.consecutive_failures + 1;
        let status = if failures >= monitor.consecutive_failures_threshold.max(1) {
            MonitorStatus::Down
        } else {
            // Not enough failures yet: keep the previous operational truth
            // (up stays up, unknown stays unknown).
            monitor.current_status
        };
        StatusTransition {
            status,
            consecutive_failures: failures,
            consecutive_successes: 0,
        }
    }
}

/// What `persist_check_outcome` did, for logging/API responses.
#[derive(Debug)]
pub struct TransitionOutcome {
    pub monitor: Monitor,
    pub incident_opened: Option<Incident>,
    pub incident_resolved: Option<Incident>,
}

/// Write the full effect of one check inside the caller's transaction:
/// open/resolve incidents if the status transition requires it, then update
/// the monitor row (counters, status, timestamps, next check).
pub async fn persist_check_outcome(
    conn: &mut SqliteConnection,
    monitor_before: &Monitor,
    transition: StatusTransition,
    failure_reason: Option<&str>,
    checked_at: DateTime<Utc>,
    next_check_at: Option<DateTime<Utc>>,
) -> Result<TransitionOutcome, sqlx::Error> {
    let mut incident_opened = None;
    let mut incident_resolved = None;

    let became_down = transition.status == MonitorStatus::Down
        && monitor_before.current_status != MonitorStatus::Down;
    let recovered_to_up = transition.status == MonitorStatus::Up
        && monitor_before.current_status == MonitorStatus::Down;

    if became_down
        && incident_repository::find_open(&mut *conn, monitor_before.id)
            .await?
            .is_none()
    {
        incident_opened = Some(
            incident_repository::open(&mut *conn, monitor_before.id, checked_at, failure_reason)
                .await?,
        );
    }
    if recovered_to_up {
        incident_resolved =
            incident_repository::resolve_open(&mut *conn, monitor_before.id, checked_at).await?;
    }

    monitor_repository::apply_check_outcome(
        &mut *conn,
        monitor_before.id,
        transition.status,
        transition.consecutive_failures,
        transition.consecutive_successes,
        checked_at,
        next_check_at,
    )
    .await?;

    let monitor = monitor_repository::find_by_id(&mut *conn, monitor_before.id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    Ok(TransitionOutcome {
        monitor,
        incident_opened,
        incident_resolved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn monitor(failures_threshold: i64, successes_threshold: i64) -> Monitor {
        let now = Utc::now();
        Monitor {
            id: Uuid::new_v4(),
            name: "test".into(),
            url: "https://example.com".into(),
            enabled: true,
            check_interval_seconds: 60,
            timeout_ms: 5_000,
            expected_status_min: 200,
            expected_status_max: 399,
            consecutive_failures_threshold: failures_threshold,
            consecutive_successes_threshold: successes_threshold,
            retry_max_attempts: 1,
            current_status: MonitorStatus::Unknown,
            consecutive_failures: 0,
            consecutive_successes: 0,
            created_at: now,
            updated_at: now,
            last_checked_at: None,
            next_check_at: None,
        }
    }

    #[test]
    fn first_failure_below_threshold_keeps_current_status() {
        let monitor = monitor(3, 1);
        let transition = plan_transition(&monitor, false);
        assert_eq!(transition.status, MonitorStatus::Unknown);
        assert_eq!(transition.consecutive_failures, 1);
        assert_eq!(transition.consecutive_successes, 0);
    }

    #[test]
    fn failure_at_threshold_marks_down() {
        let mut monitor = monitor(2, 1);
        monitor.current_status = MonitorStatus::Up;
        monitor.consecutive_failures = 1;
        let transition = plan_transition(&monitor, false);
        assert_eq!(transition.status, MonitorStatus::Down);
        assert_eq!(transition.consecutive_failures, 2);
    }

    #[test]
    fn recovery_requires_success_threshold() {
        let mut monitor = monitor(1, 2);
        monitor.current_status = MonitorStatus::Down;
        monitor.consecutive_failures = 3;

        let first = plan_transition(&monitor, true);
        assert_eq!(
            first.status,
            MonitorStatus::Down,
            "one success is not enough"
        );
        assert_eq!(first.consecutive_successes, 1);
        assert_eq!(first.consecutive_failures, 0);

        monitor.consecutive_successes = first.consecutive_successes;
        let second = plan_transition(&monitor, true);
        assert_eq!(second.status, MonitorStatus::Up);
        assert_eq!(second.consecutive_successes, 2);
    }

    #[test]
    fn success_resets_failure_counter() {
        let mut monitor = monitor(3, 1);
        monitor.current_status = MonitorStatus::Up;
        monitor.consecutive_failures = 2;
        let transition = plan_transition(&monitor, true);
        assert_eq!(transition.status, MonitorStatus::Up);
        assert_eq!(transition.consecutive_failures, 0);
        assert_eq!(transition.consecutive_successes, 1);
    }

    #[test]
    fn failure_resets_success_counter() {
        let mut monitor = monitor(1, 3);
        monitor.current_status = MonitorStatus::Up;
        monitor.consecutive_successes = 2;
        let transition = plan_transition(&monitor, false);
        assert_eq!(transition.status, MonitorStatus::Down);
        assert_eq!(transition.consecutive_successes, 0);
        assert_eq!(transition.consecutive_failures, 1);
    }
}
