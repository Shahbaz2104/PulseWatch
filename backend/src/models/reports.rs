//! Report DTOs returned by the checks and reports endpoints.
//!
//! Metric definitions (also documented in the README):
//! - Uptime is **check-based**: successful completed checks / eligible
//!   completed checks in the window. `null` when the window has no checks —
//!   never coerced to 100.
//! - Response time is milliseconds until response headers arrive. Checks
//!   without a response (transport failures) are excluded from latency stats,
//!   never treated as zero.
//! - Median/p95 use nearest-rank percentiles over successful checks with a
//!   recorded response time; `null` when fewer than 2 samples exist.
//! - Downtime is derived from incident timestamps: resolved incidents use
//!   `resolved_at - started_at`; open incidents use `now - started_at`.
//!   Paused intervals inside an incident are excluded.
//! - Paused seconds are clipped to the selected window and measured per
//!   monitor; they explain gaps without changing check-based uptime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::ReportWindow;

/// One time bucket of check activity (hourly or daily, UTC-aligned).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TimeBucket {
    pub bucket_start: DateTime<Utc>,
    pub checks: i64,
    pub successes: i64,
    /// `null` when the bucket has no checks.
    pub uptime_percent: Option<f64>,
    /// `null` when no check in the bucket has a recorded response time.
    pub avg_response_time_ms: Option<f64>,
}

/// Per-monitor uptime figures inside a window.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MonitorUptime {
    pub monitor_id: Uuid,
    pub name: String,
    pub url: String,
    pub current_status: String,
    pub checks: i64,
    pub successes: i64,
    /// Monitor-time paused inside the window, clipped to its bounds.
    pub paused_seconds: i64,
    /// `null` when the window has no checks.
    pub uptime_percent: Option<f64>,
}

/// Aggregated uptime report across monitors.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UptimeReport {
    pub window: ReportWindow,
    pub monitors: Vec<MonitorUptime>,
    pub total_checks: i64,
    pub total_successes: i64,
    /// Sum of per-monitor paused time clipped to the window.
    pub total_paused_seconds: i64,
    /// `null` when no monitor has checks in the window.
    pub overall_uptime_percent: Option<f64>,
}

/// Per-monitor latency figures inside a window.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MonitorResponseTimes {
    pub monitor_id: Uuid,
    pub name: String,
    /// Successful checks with a recorded response time.
    pub samples: i64,
    pub avg_response_time_ms: Option<f64>,
    /// `null` when fewer than 2 samples.
    pub median_response_time_ms: Option<f64>,
    /// `null` when fewer than 2 samples.
    pub p95_response_time_ms: Option<f64>,
}

/// Latency report for charts and comparisons.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResponseTimeReport {
    pub window: ReportWindow,
    /// Bucket granularity used for `series` (`hour` or `day`).
    pub bucket: String,
    pub monitors: Vec<MonitorResponseTimes>,
    /// Global (all monitors) time series.
    pub series: Vec<TimeBucket>,
}

/// Monitor status census for the overview dashboard.
#[derive(Debug, Clone, Copy, Default, Serialize, ToSchema)]
pub struct MonitorCounts {
    pub total: i64,
    pub up: i64,
    pub down: i64,
    pub unknown: i64,
    pub paused: i64,
}

/// Compact recent-activity row for the overview.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecentCheck {
    pub monitor_id: Uuid,
    pub monitor_name: String,
    pub checked_at: DateTime<Utc>,
    pub success: bool,
    pub http_status_code: Option<i64>,
    pub response_time_ms: Option<i64>,
}

/// Incident statistics for a window.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct IncidentStats {
    /// Incidents that started inside the window.
    pub outage_count: i64,
    /// Sum of incident durations, excluding paused overlap (open incidents
    /// measured at query time).
    pub downtime_seconds: i64,
}

/// Full overview dashboard payload.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OverviewReport {
    pub window: ReportWindow,
    pub monitors: MonitorCounts,
    /// Check-based uptime across all monitors; `null` when no checks.
    pub uptime_percent: Option<f64>,
    /// `null` when no check in the window has a response time.
    pub avg_response_time_ms: Option<f64>,
    pub incidents: IncidentStats,
    /// Newest incidents first, capped at 5.
    pub recent_incidents: Vec<crate::models::incident::IncidentWithMonitor>,
    /// Newest checks first, capped at 10.
    pub recent_checks: Vec<RecentCheck>,
    /// Hourly uptime sparkline for the last 24 hours.
    pub uptime_series: Vec<TimeBucket>,
    /// Response-time history over the requested window.
    pub response_time_series: Vec<TimeBucket>,
}

/// `GET /reports/incidents` payload: windowed incident stats + list.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IncidentReport {
    pub window: ReportWindow,
    pub stats: IncidentStats,
    /// Incidents that started inside the window, newest first.
    pub incidents: Vec<crate::models::incident::IncidentWithMonitor>,
}

/// Query for reports endpoints (window + optional monitor scope).
#[derive(Debug, Default, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ReportQuery {
    pub window: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// Restrict response-time/uptime per-monitor data to one monitor.
    pub monitor_id: Option<Uuid>,
}
