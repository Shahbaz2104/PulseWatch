//! HTTP routing: API routes, the OpenAPI UI, CORS and tracing layers.

use axum::{
    Router,
    http::{HeaderValue, Method},
    routing::get,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use swagger_ui_dist::{ApiDefinition, OpenApiSource, generate_routes};

use crate::openapi::openapi_json;
use crate::state::AppState;

pub mod health;

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
        tracing::warn!("no valid ALLOWED_ORIGINS configured; cross-origin requests will be blocked");
    } else {
        layer = layer.allow_origin(parsed);
    }
    layer
}
