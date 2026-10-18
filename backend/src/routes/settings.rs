//! Application settings endpoints: effective defaults (env + DB overrides)
//! and partial updates with restart-required reporting.

use axum::{extract::State, Json};

use crate::error::AppError;
use crate::services::settings_service::{AppSettings, SettingsPatch, SettingsUpdateResponse};
use crate::state::AppState;

/// Get effective application settings.
#[utoipa::path(
    get,
    path = "/api/v1/settings",
    tag = "settings",
    responses(
        (status = 200, description = "Effective settings (env defaults + stored overrides)", body = AppSettings),
    )
)]
pub async fn get(State(state): State<AppState>) -> Result<Json<AppSettings>, AppError> {
    let settings = AppSettings::load(&state.pool, &state.config)
        .await
        .map_err(AppError::Database)?;
    Ok(Json(settings))
}

/// Update settings. Only provided fields change; the response lists keys that
/// only take effect after restarting the process (currently the scheduler
/// semaphore size).
#[utoipa::path(
    put,
    path = "/api/v1/settings",
    tag = "settings",
    request_body = SettingsPatch,
    responses(
        (status = 200, description = "Updated settings plus restart-required keys", body = SettingsUpdateResponse),
        (status = 422, description = "Validation failed", body = crate::error::ErrorResponse),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    Json(patch): Json<SettingsPatch>,
) -> Result<Json<SettingsUpdateResponse>, AppError> {
    let response = AppSettings::update(&state.pool, &state.config, patch).await?;
    Ok(Json(response))
}
