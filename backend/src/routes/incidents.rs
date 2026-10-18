//! Incident listing and detail endpoints.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::incident::{IncidentWithMonitor, ListIncidentsQuery};
use crate::models::parse_rfc3339;
use crate::repositories::incident_repository;
use crate::routes::monitors::Pagination;
use crate::services::report_service;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct PaginatedIncidents {
    pub data: Vec<IncidentWithMonitor>,
    pub pagination: Pagination,
}

/// Incident list with status/monitor/date filters and pagination.
#[utoipa::path(
    get,
    path = "/api/v1/incidents",
    tag = "incidents",
    params(ListIncidentsQuery),
    responses(
        (status = 200, description = "Paginated incident list", body = PaginatedIncidents),
        (status = 422, description = "Invalid query parameters", body = crate::error::ErrorResponse),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListIncidentsQuery>,
) -> Result<Json<PaginatedIncidents>, AppError> {
    let status = report_service::parse_incident_status(query.status.as_deref())?;
    let from = query
        .from
        .as_deref()
        .map(|value| parse_rfc3339("from", value))
        .transpose()?;
    let to = query
        .to
        .as_deref()
        .map(|value| parse_rfc3339("to", value))
        .transpose()?;
    if let (Some(from), Some(to)) = (from, to) {
        if from >= to {
            return Err(AppError::validation("from", "must be earlier than to"));
        }
    }
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);

    let (data, total) = incident_repository::list_with_monitor(
        &state.pool,
        status,
        query.monitor_id,
        from,
        to,
        page,
        page_size,
    )
    .await?;
    Ok(Json(PaginatedIncidents {
        data,
        pagination: Pagination::new(page, page_size, total),
    }))
}

/// One incident joined with its monitor and computed duration.
#[utoipa::path(
    get,
    path = "/api/v1/incidents/{id}",
    tag = "incidents",
    params(("id" = Uuid, Path, description = "Incident id")),
    responses(
        (status = 200, description = "Incident details", body = IncidentWithMonitor),
        (status = 404, description = "Incident not found", body = crate::error::ErrorResponse),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IncidentWithMonitor>, AppError> {
    let incident = incident_repository::find_with_monitor(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("incident not found".to_string()))?;
    Ok(Json(incident))
}
