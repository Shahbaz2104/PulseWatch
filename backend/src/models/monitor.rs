//! Monitor entity and related enums/DTOs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Lifecycle status of a monitor.
///
/// `Paused` monitors never run checks and must never be reported as
/// operational regardless of their last check outcome.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum MonitorStatus {
    Unknown,
    Up,
    Down,
    Paused,
}

impl MonitorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MonitorStatus::Unknown => "unknown",
            MonitorStatus::Up => "up",
            MonitorStatus::Down => "down",
            MonitorStatus::Paused => "paused",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unknown" => Some(MonitorStatus::Unknown),
            "up" => Some(MonitorStatus::Up),
            "down" => Some(MonitorStatus::Down),
            "paused" => Some(MonitorStatus::Paused),
            _ => None,
        }
    }
}

/// Row + domain representation of a monitored URL.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Monitor {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub check_interval_seconds: i64,
    pub timeout_ms: i64,
    pub expected_status_min: i64,
    pub expected_status_max: i64,
    pub consecutive_failures_threshold: i64,
    pub consecutive_successes_threshold: i64,
    pub retry_max_attempts: i64,
    pub current_status: MonitorStatus,
    pub consecutive_failures: i64,
    pub consecutive_successes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub next_check_at: Option<DateTime<Utc>>,
}

/// Request body for `POST /api/v1/monitors`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateMonitorRequest {
    #[serde(default)]
    pub name: Option<String>,
    pub url: String,
    #[serde(default)]
    pub check_interval_seconds: Option<i64>,
    #[serde(default)]
    pub timeout_ms: Option<i64>,
    #[serde(default)]
    pub expected_status_min: Option<i64>,
    #[serde(default)]
    pub expected_status_max: Option<i64>,
    #[serde(default)]
    pub consecutive_failures_threshold: Option<i64>,
    #[serde(default)]
    pub consecutive_successes_threshold: Option<i64>,
    #[serde(default)]
    pub retry_max_attempts: Option<i64>,
}

/// Request body for `PUT /api/v1/monitors/{id}` (all fields optional).
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateMonitorRequest {
    pub name: Option<String>,
    pub url: Option<String>,
    pub check_interval_seconds: Option<i64>,
    pub timeout_ms: Option<i64>,
    pub expected_status_min: Option<i64>,
    pub expected_status_max: Option<i64>,
    pub consecutive_failures_threshold: Option<i64>,
    pub consecutive_successes_threshold: Option<i64>,
    pub retry_max_attempts: Option<i64>,
}

/// Monitor fields plus latest check snapshot and windowed uptime for list views.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MonitorSummary {
    #[serde(flatten)]
    pub monitor: Monitor,
    /// Status of the most recent completed check, if any.
    pub latest_http_status: Option<i64>,
    pub latest_response_time_ms: Option<i64>,
    pub latest_success: Option<bool>,
    /// Check-based uptime percentage inside the requested window;
    /// `null` when the window contains no completed checks.
    pub uptime_percent: Option<f64>,
    pub checks_in_window: i64,
    /// Derives an amber "degraded" hint for the UI: nominally up, but the most
    /// recent checks have not yet met the recovery threshold.
    pub degraded: bool,
}

impl From<Monitor> for MonitorSummary {
    fn from(monitor: Monitor) -> Self {
        Self {
            monitor,
            latest_http_status: None,
            latest_response_time_ms: None,
            latest_success: None,
            uptime_percent: None,
            checks_in_window: 0,
            degraded: false,
        }
    }
}

/// Query parameters accepted by monitor list endpoints.
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListMonitorsQuery {
    /// Free-text match against name or URL.
    pub search: Option<String>,
    /// Filter by `unknown|up|down|paused`.
    pub status: Option<String>,
    /// Reporting window: `24h`, `7d`, `30d` or `custom`.
    pub window: Option<String>,
    /// Custom window start (RFC3339, required when `window=custom`).
    pub from: Option<String>,
    /// Custom window end (RFC3339).
    pub to: Option<String>,
    /// Sort field: `name`, `uptime`, `response_time`, `last_checked`, `created`.
    pub sort: Option<String>,
    /// Sort direction: `asc` or `desc`.
    pub order: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl Default for ListMonitorsQuery {
    fn default() -> Self {
        Self {
            search: None,
            status: None,
            window: Some("24h".to_string()),
            from: None,
            to: None,
            sort: None,
            order: None,
            page: None,
            page_size: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_sqlite_text_form() {
        for status in [
            MonitorStatus::Unknown,
            MonitorStatus::Up,
            MonitorStatus::Down,
            MonitorStatus::Paused,
        ] {
            assert_eq!(MonitorStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(MonitorStatus::parse("nope"), None);
    }

    #[test]
    fn status_serde_is_lowercase() {
        let json = serde_json::to_string(&MonitorStatus::Down).unwrap();
        assert_eq!(json, "\"down\"");
        let parsed: MonitorStatus = serde_json::from_str("\"paused\"").unwrap();
        assert_eq!(parsed, MonitorStatus::Paused);
    }
}
