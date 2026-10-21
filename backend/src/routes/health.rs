//! Health endpoints.

use axum::{extract::State, http::StatusCode, Json};
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
    /// Whether SMTP email delivery is configured (never exposes credentials).
    pub email_configured: bool,
    /// Read-only public demo: auth is bypassed and every mutation is rejected.
    pub demo_mode: bool,
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
    let database = match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
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
                email_configured: state.config.email_configured(),
                demo_mode: state.config.demo_mode,
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
                email_configured: state.config.email_configured(),
                demo_mode: state.config.demo_mode,
            }),
        )
    }
}
