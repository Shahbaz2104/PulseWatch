//! Report endpoints: overview dashboard, uptime, response times, incidents.

use axum::{
    extract::{Query, State},
    Json,
};

use crate::error::AppError;
use crate::models::reports::{IncidentReport, ReportQuery, ResponseTimeReport, UptimeReport};
use crate::models::ReportWindow;
use crate::services::report_service;
use crate::state::AppState;

fn window_from(query: &ReportQuery) -> Result<ReportWindow, AppError> {
    ReportWindow::from_query(
        query.window.as_deref(),
        query.from.as_deref(),
        query.to.as_deref(),
    )
}

/// Overview dashboard: status census, overall uptime, incident stats, recent
/// activity, 24h uptime sparkline and response-time history.
#[utoipa::path(
    get,
    path = "/api/v1/reports/overview",
    tag = "reports",
    params(ReportQuery),
    responses(
        (status = 200, description = "Overview dashboard payload", body = crate::models::reports::OverviewReport),
        (status = 422, description = "Invalid window parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn overview(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<crate::models::reports::OverviewReport>, AppError> {
    let window = window_from(&query)?;
    Ok(Json(report_service::overview(&state, window).await?))
}

/// Per-monitor uptime inside the selected window.
#[utoipa::path(
    get,
    path = "/api/v1/reports/uptime",
    tag = "reports",
    params(ReportQuery),
    responses(
        (status = 200, description = "Uptime report", body = UptimeReport),
        (status = 422, description = "Invalid window parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn uptime(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<UptimeReport>, AppError> {
    let window = window_from(&query)?;
    Ok(Json(report_service::uptime_report(&state, window).await?))
}

/// Per-monitor latency percentiles plus a bucketed response-time series.
#[utoipa::path(
    get,
    path = "/api/v1/reports/response-times",
    tag = "reports",
    params(ReportQuery),
    responses(
        (status = 200, description = "Response-time report", body = ResponseTimeReport),
        (status = 404, description = "Requested monitor not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Invalid window parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn response_times(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<ResponseTimeReport>, AppError> {
    let window = window_from(&query)?;
    Ok(Json(
        report_service::response_times(&state, window, query.monitor_id).await?,
    ))
}

/// Windowed incident statistics plus the incidents themselves.
#[utoipa::path(
    get,
    path = "/api/v1/reports/incidents",
    tag = "reports",
    params(ReportQuery),
    responses(
        (status = 200, description = "Incident report", body = IncidentReport),
        (status = 422, description = "Invalid window parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn incidents(
    State(state): State<AppState>,
    Query(query): Query<ReportQuery>,
) -> Result<Json<IncidentReport>, AppError> {
    let window = window_from(&query)?;
    Ok(Json(report_service::incident_report(&state, window).await?))
}
