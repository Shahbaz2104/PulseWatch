//! Shared integration-test harness: per-test temp-file SQLite database,
//! test config, SSRF-safe HTTP client, and an in-process router driven via
//! `tower::ServiceExt::oneshot` (no TCP listener needed).
//
// Each test binary compiles this module separately, so helpers used by only
// one suite would otherwise trip dead-code lints in the others.
#![allow(dead_code)]

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use pulsewatch_backend::{
    config::Config,
    db, routes,
    state::{AppState, InFlightSet},
    validation::build_safe_http_client,
};
use serde_json::Value;
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

pub struct TestApp {
    pub state: AppState,
    pub router: Router,
    /// Kept alive for the lifetime of the test so the temp DB is not deleted.
    _dir: TempDir,
}

impl TestApp {
    /// Builds a fully wired app against a fresh temporary database.
    /// `allow_private_targets` must be `true` for wiremock (loopback) targets.
    pub async fn new(allow_private_targets: bool) -> Self {
        // Best-effort tracing so server-side failures surface in test output.
        static TRACING: std::sync::Once = std::sync::Once::new();
        TRACING.call_once(|| {
            let filter = tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error"));
            let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
        });
        let dir = tempfile::tempdir().expect("create temp dir");
        let database_url = format!("sqlite://{}/pulsewatch.db?mode=rwc", dir.path().display());
        let pool = db::connect(&database_url).await.expect("connect + migrate");

        let config = Arc::new(Config {
            host: "127.0.0.1".parse().unwrap(),
            port: 3000,
            database_url,
            default_check_interval_seconds: 60,
            default_timeout_ms: 5_000,
            default_retry_max_attempts: 2,
            // Small backoffs keep retry tests fast.
            default_retry_backoff_ms: 20,
            default_retry_backoff_max_ms: 50,
            max_concurrent_checks: 8,
            allow_private_targets,
            allowed_origins: vec!["http://localhost:5173".to_string()],
        });

        let http_client = build_safe_http_client(&config).expect("build safe http client");
        let state = AppState {
            pool,
            config,
            http_client: Arc::new(http_client),
            semaphore: Arc::new(Semaphore::new(8)),
            in_flight: Arc::new(InFlightSet::default()),
            shutdown: CancellationToken::new(),
        };
        let router = routes::router(state.clone());
        TestApp {
            state,
            router,
            _dir: dir,
        }
    }

    /// Sends a request and returns `(status, parsed JSON body)` (`Value::Null`
    /// for empty bodies such as 204).
    pub async fn request_json(&self, req: Request<Body>) -> (StatusCode, Value) {
        let response = self.router.clone().oneshot(req).await.expect("router call");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("JSON body")
        };
        (status, value)
    }

    pub async fn request(&self, req: Request<Body>) -> StatusCode {
        let response = self.router.clone().oneshot(req).await.expect("router call");
        response.status()
    }
}

pub fn json_request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    let body = match body {
        Some(value) => Body::from(serde_json::to_vec(&value).expect("serialize body")),
        None => Body::empty(),
    };
    builder.body(body).expect("build request")
}

pub fn target_url(server: &wiremock::MockServer, route: &str) -> String {
    format!("{}{}", server.uri(), route)
}

/// Minimal valid create body; callers override fields via `extra`.
pub fn create_body(url: &str, extra: Value) -> Value {
    let mut body = serde_json::json!({ "url": url });
    match extra {
        Value::Object(map) => {
            for (key, value) in map {
                body[key] = value;
            }
        }
        _ => panic!("extra must be a JSON object"),
    }
    body
}

/// Inserts a monitor row directly, bypassing API validation (used to reach
/// request-time SSRF layers that create-time validation would block first).
///
/// UUIDs must be bound as `Uuid` (not strings): sqlx 0.9's SQLite driver
/// encodes them as 16-byte BLOBs, so a TEXT bind would never match.
pub async fn insert_monitor_raw(
    pool: &sqlx::SqlitePool,
    id: uuid::Uuid,
    url: &str,
    retry_max_attempts: i64,
) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO monitors (
            id, name, url, enabled, check_interval_seconds, timeout_ms,
            expected_status_min, expected_status_max,
            consecutive_failures_threshold, consecutive_successes_threshold,
            retry_max_attempts, current_status, consecutive_failures,
            consecutive_successes, created_at, updated_at, last_checked_at, next_check_at
         ) VALUES (
            ?, 'raw-monitor', ?, 1, 60, 5000,
            200, 399, 1, 1,
            ?, 'unknown', 0, 0, ?, ?, NULL, NULL
         )",
    )
    .bind(id)
    .bind(url)
    .bind(retry_max_attempts)
    .bind(now.clone())
    .bind(now)
    .execute(pool)
    .await
    .expect("insert monitor row");
}

pub async fn open_incident_count(pool: &sqlx::SqlitePool, monitor_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM incidents WHERE monitor_id = ? AND status = 'open'")
        .bind(monitor_id)
        .fetch_one(pool)
        .await
        .expect("count open incidents")
}

pub async fn resolved_incident_count(pool: &sqlx::SqlitePool, monitor_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM incidents WHERE monitor_id = ? AND status = 'resolved'",
    )
    .bind(monitor_id)
    .fetch_one(pool)
    .await
    .expect("count resolved incidents")
}
