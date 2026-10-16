//! Health endpoints.

use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// `ok` when the service and database are reachable, else `degraded`.
    pub status: &'static str,
    pub service: &'static str,
    pub version: &'static str,
    /// `ok` or `unavailable` — no configuration details are exposed.
    pub database: &'static str,
}

/// Service health probe. Also mounted at `/api/v1/health`.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "health",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse),
        (status = 503, description = "Service degraded", body = HealthResponse),
    )
)]
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let database = match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => "ok",
        Err(err) => {
            tracing::error!(error = %err, "health check: database unavailable");
            "unavailable"
        }
    };

    if database == "ok" {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                service: "pulsewatch",
                version: env!("CARGO_PKG_VERSION"),
                database,
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded",
                service: "pulsewatch",
                version: env!("CARGO_PKG_VERSION"),
                database,
            }),
        )
    }
}
