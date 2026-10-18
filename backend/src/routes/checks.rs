//! Check history endpoints: paginated history, latest check, windowed summary.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::check_result::{CheckResult, CheckSummary, ListChecksQuery};
use crate::models::reports::ReportQuery;
use crate::models::ReportWindow;
use crate::repositories::{check_repository, monitor_repository};
use crate::routes::monitors::Pagination;
use crate::services::report_service;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedChecks {
    pub data: Vec<CheckResult>,
    pub pagination: Pagination,
}

async fn require_monitor(state: &AppState, id: Uuid) -> Result<(), AppError> {
    monitor_repository::find_by_id(&state.pool, id)
        .await?
        .map(|_| ())
        .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))
}

fn parse_check_filter(query: &ListChecksQuery) -> Result<check_repository::CheckFilter, AppError> {
    let from = query
        .from
        .as_deref()
        .map(|value| crate::models::parse_rfc3339("from", value))
        .transpose()?;
    let to = query
        .to
        .as_deref()
        .map(|value| crate::models::parse_rfc3339("to", value))
        .transpose()?;
    if let (Some(from), Some(to)) = (from, to) {
        if from >= to {
            return Err(AppError::validation("from", "must be earlier than to"));
        }
    }
    let success = match query.result.as_deref().map(str::trim) {
        None | Some("") => None,
        Some("success") => Some(true),
        Some("failure") => Some(false),
        Some(other) => {
            return Err(AppError::validation(
                "result",
                format!("unknown value '{other}', expected success or failure"),
            ));
        }
    };
    Ok(check_repository::CheckFilter {
        from,
        to,
        success,
        page: query.page.unwrap_or(1).max(1),
        page_size: query.page_size.unwrap_or(50).clamp(1, 200),
    })
}

/// Paginated check history for one monitor (newest first).
#[utoipa::path(
    get,
    path = "/api/v1/monitors/{id}/checks",
    tag = "checks",
    params(
        ("id" = Uuid, Path, description = "Monitor id"),
        ListChecksQuery,
    ),
    responses(
        (status = 200, description = "Paginated check history", body = PaginatedChecks),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Invalid query parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ListChecksQuery>,
) -> Result<Json<PaginatedChecks>, AppError> {
    require_monitor(&state, id).await?;
    let filter = parse_check_filter(&query)?;
    let (data, total) = check_repository::list_for_monitor(&state.pool, id, &filter).await?;
    Ok(Json(PaginatedChecks {
        data,
        pagination: Pagination::new(filter.page, filter.page_size, total),
    }))
}

/// Most recent completed check for a monitor.
#[utoipa::path(
    get,
    path = "/api/v1/monitors/{id}/checks/latest",
    tag = "checks",
    params(("id" = Uuid, Path, description = "Monitor id")),
    responses(
        (status = 200, description = "Latest check", body = CheckResult),
        (status = 404, description = "Monitor not found or no checks yet", body = crate::error::ErrorResponse),
    )
)]
pub async fn latest(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckResult>, AppError> {
    require_monitor(&state, id).await?;
    let check = check_repository::latest_for_monitor(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("no checks recorded yet".to_string()))?;
    Ok(Json(check))
}

/// Windowed summary (uptime + latency percentiles) for one monitor.
#[utoipa::path(
    get,
    path = "/api/v1/monitors/{id}/checks/summary",
    tag = "checks",
    params(
        ("id" = Uuid, Path, description = "Monitor id"),
        ReportQuery,
    ),
    responses(
        (status = 200, description = "Windowed check summary", body = CheckSummary),
        (status = 404, description = "Monitor not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Invalid window parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn summary(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<CheckSummary>, AppError> {
    require_monitor(&state, id).await?;
    let window = ReportWindow::from_query(
        query.window.as_deref(),
        query.from.as_deref(),
        query.to.as_deref(),
    )?;

    let stats =
        monitor_repository::window_stats(&state.pool, &[id], window.start, window.end).await?;
    let stat = stats.get(&id);
    let total_checks = stat.map(|s| s.checks).unwrap_or(0);
    let successful_checks = stat.map(|s| s.successes).unwrap_or(0);
    let uptime_percent = report_service::uptime_percent(total_checks, successful_checks);

    let (samples, avg, median, p95) =
        check_repository::latency_stats(&state.pool, id, window.start, window.end).await?;
    debug_assert!(samples >= 0);
    let latest_response_time_ms = check_repository::latest_for_monitor(&state.pool, id)
        .await?
        .and_then(|check| check.response_time_ms);

    Ok(Json(CheckSummary {
        total_checks,
        successful_checks,
        uptime_percent,
        latest_response_time_ms,
        avg_response_time_ms: avg,
        median_response_time_ms: median.map(|value| value as f64),
        p95_response_time_ms: p95.map(|value| value as f64),
        window_start: window.start,
        window_end: window.end,
    }))
}
