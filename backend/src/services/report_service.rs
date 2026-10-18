//! Report aggregation: pure metric math plus orchestration of the
//! repository queries. Definitions:
//!
//! - Uptime is check-based (successful / eligible completed checks). `None`
//!   when the window has no checks — never 100 by default.
//! - Latency stats use only successful checks with a recorded response time.
//!   Median/p95 are exact nearest-rank percentiles; `None` below 2 samples.
//! - Downtime sums incident durations; open incidents are measured at query
//!   time. Only incidents *started* inside the window contribute.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::incident::IncidentStatus;
use crate::models::monitor::MonitorStatus;
use crate::models::reports::{
    IncidentReport, IncidentStats, MonitorCounts, MonitorResponseTimes, MonitorUptime,
    OverviewReport, RecentCheck, ResponseTimeReport, TimeBucket, UptimeReport,
};
use crate::models::ReportWindow;
use crate::repositories::{check_repository, incident_repository, monitor_repository};
use crate::state::AppState;

/// 0-based offset of the nearest-rank percentile `p` (0 < p <= 1) over `n`
/// samples: `ceil(p * n) - 1`, clamped to `[0, n - 1]`.
pub fn nearest_rank_offset(p: f64, n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let rank = (p * n as f64).ceil() as i64;
    rank.clamp(1, n) - 1
}

/// Check-based uptime percentage; `None` when there are no eligible checks.
pub fn uptime_percent(checks: i64, successes: i64) -> Option<f64> {
    if checks <= 0 {
        return None;
    }
    Some(successes as f64 / checks as f64 * 100.0)
}

/// Whether a window should be bucketed daily (`true`) or hourly (`false`).
pub fn bucket_daily(window: &ReportWindow) -> bool {
    window.duration_hours() > 24.0
}

/// Parse a strftime bucket label (`%Y-%m-%dT%H:00:00Z`) back to a timestamp.
fn parse_bucket(label: &str) -> Result<DateTime<Utc>, AppError> {
    label
        .parse()
        .map_err(|_| AppError::Internal(format!("invalid bucket label {label}")))
}

fn to_time_buckets(
    rows: Vec<(String, i64, i64, Option<f64>)>,
) -> Result<Vec<TimeBucket>, AppError> {
    rows.into_iter()
        .map(|(label, checks, successes, avg_response_time_ms)| {
            Ok(TimeBucket {
                bucket_start: parse_bucket(&label)?,
                checks,
                successes,
                uptime_percent: uptime_percent(checks, successes),
                avg_response_time_ms,
            })
        })
        .collect()
}

/// Downtime totals for incidents started inside the window.
async fn incident_stats(state: &AppState, window: ReportWindow) -> Result<IncidentStats, AppError> {
    let rows = incident_repository::window_incidents(&state.pool, window.start, window.end).await?;
    let now = Utc::now();
    let mut outage_count = 0i64;
    let mut downtime_seconds = 0i64;
    for (started_at, resolved_at) in rows {
        let end = resolved_at.unwrap_or(now);
        downtime_seconds += (end - started_at).num_seconds().max(0);
        outage_count += 1;
    }
    Ok(IncidentStats {
        outage_count,
        downtime_seconds,
    })
}

/// Overview dashboard payload.
pub async fn overview(state: &AppState, window: ReportWindow) -> Result<OverviewReport, AppError> {
    let counts = monitor_repository::status_counts(&state.pool).await?;
    let mut monitors = MonitorCounts::default();
    for (status, count) in counts {
        monitors.total += count;
        match MonitorStatus::parse(&status) {
            Some(MonitorStatus::Up) => monitors.up += count,
            Some(MonitorStatus::Down) => monitors.down += count,
            Some(MonitorStatus::Unknown) => monitors.unknown += count,
            Some(MonitorStatus::Paused) => monitors.paused += count,
            None => {}
        }
    }

    let per_monitor =
        check_repository::window_counts(&state.pool, window.start, window.end).await?;
    let total_checks: i64 = per_monitor.iter().map(|(_, checks, _)| *checks).sum();
    let total_successes: i64 = per_monitor.iter().map(|(_, _, successes)| *successes).sum();
    let uptime_percent = uptime_percent(total_checks, total_successes);

    // Average response time across all monitors with a recorded response.
    let avg_response_time_ms: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(response_time_ms) FROM check_results \
         WHERE checked_at >= ? AND checked_at < ? AND success = 1 AND response_time_ms IS NOT NULL",
    )
    .bind(window.start)
    .bind(window.end)
    .fetch_one(&state.pool)
    .await?;

    let incidents = incident_stats(state, window).await?;
    let recent_incidents = incident_repository::recent(&state.pool, 5).await?;
    let recent_checks = check_repository::recent_global(&state.pool, 10)
        .await?
        .into_iter()
        .map(
            |(
                monitor_id,
                monitor_name,
                checked_at,
                success,
                http_status_code,
                response_time_ms,
            )| {
                RecentCheck {
                    monitor_id,
                    monitor_name,
                    checked_at,
                    success,
                    http_status_code,
                    response_time_ms,
                }
            },
        )
        .collect();

    // Fixed 24h hourly uptime sparkline (spec: "uptime summary over the last
    // 24 hours"), independent of the selected window.
    let spark_end = Utc::now();
    let spark_start = spark_end - chrono::Duration::hours(24);
    let uptime_series = to_time_buckets(
        check_repository::activity_series(&state.pool, None, spark_start, spark_end, false).await?,
    )?;

    // Both sparklines share the same fixed 24h hourly base so the overview
    // payload stays time-base consistent; windowed series live on
    // /reports/response-times instead.
    let response_time_series = to_time_buckets(
        check_repository::activity_series(&state.pool, None, spark_start, spark_end, false).await?,
    )?;

    Ok(OverviewReport {
        window,
        monitors,
        uptime_percent,
        avg_response_time_ms,
        incidents,
        recent_incidents,
        recent_checks,
        uptime_series,
        response_time_series,
    })
}

/// Per-monitor uptime report.
pub async fn uptime_report(
    state: &AppState,
    window: ReportWindow,
) -> Result<UptimeReport, AppError> {
    let monitors = monitor_repository::find_all(&state.pool).await?;
    let counts = check_repository::window_counts(&state.pool, window.start, window.end).await?;
    let by_id: std::collections::HashMap<Uuid, (i64, i64)> = counts
        .into_iter()
        .map(|(id, checks, successes)| (id, (checks, successes)))
        .collect();

    let mut total_checks = 0i64;
    let mut total_successes = 0i64;
    let monitors: Vec<MonitorUptime> = monitors
        .into_iter()
        .map(|monitor| {
            let (checks, successes) = by_id.get(&monitor.id).copied().unwrap_or((0, 0));
            total_checks += checks;
            total_successes += successes;
            MonitorUptime {
                monitor_id: monitor.id,
                name: monitor.name,
                url: monitor.url,
                current_status: monitor.current_status.as_str().to_string(),
                checks,
                successes,
                uptime_percent: uptime_percent(checks, successes),
            }
        })
        .collect();

    Ok(UptimeReport {
        window,
        monitors,
        total_checks,
        total_successes,
        overall_uptime_percent: uptime_percent(total_checks, total_successes),
    })
}

/// Per-monitor latency figures plus a global time series.
pub async fn response_times(
    state: &AppState,
    window: ReportWindow,
    monitor_id: Option<Uuid>,
) -> Result<ResponseTimeReport, AppError> {
    // Validate an explicitly requested monitor exists (404 otherwise).
    if let Some(id) = monitor_id {
        monitor_repository::find_by_id(&state.pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))?;
    }

    let monitors = match monitor_id {
        Some(id) => {
            let monitor = monitor_repository::find_by_id(&state.pool, id)
                .await?
                .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))?;
            vec![monitor]
        }
        None => monitor_repository::find_all(&state.pool).await?,
    };

    let mut per_monitor = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let (samples, avg, median, p95) =
            check_repository::latency_stats(&state.pool, monitor.id, window.start, window.end)
                .await?;
        per_monitor.push(MonitorResponseTimes {
            monitor_id: monitor.id,
            name: monitor.name,
            samples,
            avg_response_time_ms: avg,
            median_response_time_ms: median.map(|value| value as f64),
            p95_response_time_ms: p95.map(|value| value as f64),
        });
    }

    let daily = bucket_daily(&window);
    let series = to_time_buckets(
        check_repository::activity_series(&state.pool, monitor_id, window.start, window.end, daily)
            .await?,
    )?;

    Ok(ResponseTimeReport {
        window,
        bucket: if daily { "day" } else { "hour" }.to_string(),
        monitors: per_monitor,
        series,
    })
}

/// Windowed incident report (stats + list).
pub async fn incident_report(
    state: &AppState,
    window: ReportWindow,
) -> Result<IncidentReport, AppError> {
    let stats = incident_stats(state, window).await?;
    let (incidents, _) = incident_repository::list_with_monitor(
        &state.pool,
        None,
        None,
        Some(window.start),
        Some(window.end),
        1,
        200,
    )
    .await?;
    Ok(IncidentReport {
        window,
        stats,
        incidents,
    })
}

/// Parse the `status` filter for incident listing.
pub fn parse_incident_status(value: Option<&str>) -> Result<Option<IncidentStatus>, AppError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(status) => IncidentStatus::parse(status)
            .map(Some)
            .ok_or_else(|| AppError::validation("status", "must be one of open, resolved")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_offsets() {
        // n=1: every percentile lands on the single sample.
        assert_eq!(nearest_rank_offset(0.5, 1), 0);
        assert_eq!(nearest_rank_offset(0.95, 1), 0);
        // n=8: median rank ceil(4)=4 → offset 3; p95 rank ceil(7.6)=8 → offset 7.
        assert_eq!(nearest_rank_offset(0.5, 8), 3);
        assert_eq!(nearest_rank_offset(0.95, 8), 7);
        // n=5: median rank ceil(2.5)=3 → offset 2; p95 rank ceil(4.75)=5 → offset 4.
        assert_eq!(nearest_rank_offset(0.5, 5), 2);
        assert_eq!(nearest_rank_offset(0.95, 5), 4);
        // n=10: median offset 4; p95 rank ceil(9.5)=10 → offset 9.
        assert_eq!(nearest_rank_offset(0.5, 10), 4);
        assert_eq!(nearest_rank_offset(0.95, 10), 9);
        assert_eq!(nearest_rank_offset(0.5, 0), 0);
    }

    #[test]
    fn uptime_null_on_empty_window() {
        assert_eq!(uptime_percent(0, 0), None);
        assert_eq!(uptime_percent(0, 5), None);
    }

    #[test]
    fn uptime_exact_fractions() {
        assert_eq!(uptime_percent(4, 3), Some(75.0));
        assert_eq!(uptime_percent(10, 9), Some(90.0));
        assert_eq!(uptime_percent(3, 3), Some(100.0));
        let value = uptime_percent(7, 5).unwrap();
        assert!((value - 71.428_571_428_571_43_f64).abs() < 1e-9);
    }

    #[test]
    fn bucket_granularity_follows_window_length() {
        let hour = ReportWindow::last_hours(24);
        assert!(!bucket_daily(&hour));
        let week = ReportWindow::last_hours(24 * 7);
        assert!(bucket_daily(&week));
        let month = ReportWindow::last_hours(24 * 30);
        assert!(bucket_daily(&month));
    }

    #[test]
    fn bucket_labels_parse_back_to_utc() {
        let parsed = parse_bucket("2026-10-18T13:00:00Z").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-10-18T13:00:00+00:00");
    }

    #[test]
    fn incident_status_filter_parses() {
        assert!(parse_incident_status(None).unwrap().is_none());
        assert!(parse_incident_status(Some("")).unwrap().is_none());
        assert!(matches!(
            parse_incident_status(Some("open")).unwrap(),
            Some(IncidentStatus::Open)
        ));
        assert!(parse_incident_status(Some("bogus")).is_err());
    }
}
