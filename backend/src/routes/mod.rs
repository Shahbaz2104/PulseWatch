//! HTTP routing: API routes, the OpenAPI UI, CORS and tracing layers.

use axum::{
    http::{HeaderValue, Method},
    routing::get,
    Router,
};
use swagger_ui_dist::{generate_routes, ApiDefinition, OpenApiSource};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::openapi::openapi_json;
use crate::state::AppState;

pub mod checks;
pub mod health;
pub mod incidents;
pub mod monitors;
pub mod reports;
pub mod settings;

pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state.config.allowed_origins);

    // The OpenAPI document is rendered once at startup; route paths are static.
    let spec_json = match serde_json::to_string(&openapi_json()) {
        Ok(spec) => spec,
        Err(err) => {
            tracing::error!(error = %err, "failed to serialize OpenAPI document");
            "{}".to_string()
        }
    };

    let api = Router::new()
        .route("/health", get(health::health))
        .route("/openapi.json", get(openapi_json_route))
        .route("/monitors", get(monitors::list).post(monitors::create))
        .route(
            "/monitors/{id}",
            get(monitors::get)
                .put(monitors::update)
                .delete(monitors::delete),
        )
        .route(
            "/monitors/{id}/check",
            axum::routing::post(monitors::check_now),
        )
        .route("/monitors/{id}/pause", axum::routing::post(monitors::pause))
        .route(
            "/monitors/{id}/resume",
            axum::routing::post(monitors::resume),
        )
        .route("/monitors/{id}/checks", get(checks::list))
        .route("/monitors/{id}/checks/latest", get(checks::latest))
        .route("/monitors/{id}/checks/summary", get(checks::summary))
        .route("/reports/overview", get(reports::overview))
        .route("/reports/uptime", get(reports::uptime))
        .route("/reports/response-times", get(reports::response_times))
        .route("/reports/incidents", get(reports::incidents))
        .route("/incidents", get(incidents::list))
        .route("/incidents/{id}", get(incidents::get))
        .route("/settings", get(settings::get).put(settings::update))
        .merge(
            generate_routes(ApiDefinition {
                uri_prefix: "/api/v1/docs",
                api_definition: OpenApiSource::Inline(spec_json.as_str()),
                title: Some("PulseWatch API"),
            })
            // Swagger UI routes are stateless; retype them so they can be
            // merged into the stateful API router (see axum `with_state` docs).
            .with_state(()),
        );

    Router::new()
        .route("/health", get(health::health))
        .nest("/api/v1", api)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn openapi_json_route() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(openapi_json())
}

/// CORS restricted to the configured frontend origins (defaults cover the
/// Vite dev server). Invalid origins are skipped; when none are valid the
/// router sends no CORS headers, so browsers only allow same-origin calls.
/// The API itself is unauthenticated and meant for local use — see README
/// "Security considerations".
fn build_cors(origins: &[String]) -> CorsLayer {
    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| origin.trim().parse().ok())
        .collect();

    if parsed.is_empty() {
        tracing::warn!(
            "no valid ALLOWED_ORIGINS configured; cross-origin requests will be blocked"
        );
    } else {
        layer = layer.allow_origin(parsed);
    }
    layer
}
