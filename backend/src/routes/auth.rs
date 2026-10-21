//! Admin authentication endpoints: login, logout and session status.

use axum::{
    extract::State,
    http::header::{CACHE_CONTROL, SET_COOKIE},
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{
    clear_cookie, cookie_header, is_authenticated, issue_session, session_cookie, verify_password,
};
use crate::error::AppError;
use crate::state::AppState;

/// Current session state. Public so the UI can decide what to render.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub demo_mode: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub password: String,
}

/// `GET /api/v1/auth/status`
#[utoipa::path(
    get,
    path = "/api/v1/auth/status",
    tag = "auth",
    responses(
        (status = 200, description = "Current session state", body = AuthStatus),
    )
)]
pub async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let mut response = Json(AuthStatus {
        authenticated: is_authenticated(&state.config, &headers),
        demo_mode: state.config.demo_mode,
    })
    .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// `POST /api/v1/auth/login`
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Session cookie issued", body = AuthStatus),
        (status = 401, description = "Invalid password", body = crate::error::ErrorResponse),
        (status = 403, description = "Demo mode is read-only", body = crate::error::ErrorResponse),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, AppError> {
    if state.config.demo_mode {
        return Err(AppError::Forbidden("demo mode is read-only".to_string()));
    }
    if !verify_password(&state.config, &request.password) {
        tracing::warn!("admin login failed: invalid password");
        return Err(AppError::Unauthorized("invalid password".to_string()));
    }

    let token = issue_session(&state.config)
        .ok_or_else(|| AppError::Internal("auth is not configured".to_string()))?;
    let mut response = Json(AuthStatus {
        authenticated: true,
        demo_mode: false,
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie_header(&session_cookie(&state.config, &token)),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    tracing::info!("admin signed in");
    Ok(response)
}

/// `POST /api/v1/auth/logout` — always clears the cookie, even when the session
/// was already invalid or expired.
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    responses(
        (status = 204, description = "Session cookie cleared"),
    )
)]
pub async fn logout(State(state): State<AppState>) -> Response {
    let mut response = axum::http::StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie_header(&clear_cookie(&state.config)));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    tracing::info!("admin signed out");
    response
}
