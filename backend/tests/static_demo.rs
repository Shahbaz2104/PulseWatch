//! Static frontend serving (SPA fallback, JSON API 404s) and demo seeding.

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use common::{insert_monitor_raw, TestApp};
use pulsewatch_backend::services::demo_service;
use serde_json::Value;
use tower::ServiceExt;

async fn get_body(app: &TestApp, uri: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("build request");
    let response = app.router.clone().oneshot(request).await.expect("router");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
async fn static_dir_serves_spa_and_keeps_api_404s_json() {
    let dir = tempfile::tempdir().expect("static dir");
    std::fs::write(
        dir.path().join("index.html"),
        "<!doctype html><title>PulseWatch Demo Shell</title>",
    )
    .expect("write index");
    std::fs::create_dir(dir.path().join("assets")).expect("assets dir");
    std::fs::write(dir.path().join("assets/app.js"), "console.log('bundle');")
        .expect("write asset");

    let app = TestApp::with_static(false, dir.path().to_path_buf()).await;

    let (status, body) = get_body(&app, "/").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "root should serve the SPA shell: {body:?}"
    );
    assert!(body.contains("PulseWatch Demo Shell"));

    // Unknown client-side routes fall back to the SPA entrypoint.
    let (status, body) = get_body(&app, "/monitors/not-a-real-id").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("PulseWatch Demo Shell"));

    // Hashed assets are served verbatim.
    let (status, body) = get_body(&app, "/assets/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("bundle"));

    // Unknown API paths stay JSON, never the SPA HTML.
    for uri in ["/api/v1/definitely-missing", "/api/missing"] {
        let (status, body) = get_body(&app, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "for {uri}");
        let parsed: Value = serde_json::from_str(&body).expect("JSON error body");
        assert_eq!(parsed["error"]["code"], "not_found", "for {uri}");
    }

    let (status, body) = get_body(&app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    let parsed: Value = serde_json::from_str(&body).expect("health JSON");
    assert_eq!(parsed["status"], "ok");
}

#[tokio::test]
async fn without_static_dir_client_routes_are_plain_404s() {
    let app = TestApp::new(false).await;
    let (status, _) = get_body(&app, "/").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn demo_seed_populates_once_and_serves_read_only_data() {
    let app = TestApp::demo(true).await;

    let first = demo_service::seed_if_empty(&app.state)
        .await
        .expect("first seed");
    assert!(first.seeded());
    assert_eq!(first.monitors, 6);
    assert_eq!(first.checks, 124);
    assert_eq!(first.incidents, 2);

    let second = demo_service::seed_if_empty(&app.state)
        .await
        .expect("second seed");
    assert_eq!(second, demo_service::SeedOutcome::default());

    let monitors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM monitors")
        .fetch_one(&app.state.pool)
        .await
        .expect("count monitors");
    assert_eq!(monitors, 6);
    let open_incidents: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM incidents WHERE resolved_at IS NULL")
            .fetch_one(&app.state.pool)
            .await
            .expect("count open incidents");
    assert_eq!(open_incidents, 1);
    let open_pauses: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM monitor_pauses WHERE resumed_at IS NULL")
            .fetch_one(&app.state.pool)
            .await
            .expect("count open pauses");
    assert_eq!(open_pauses, 1);

    let (status, body) = app
        .request_json_anon(common::json_request("GET", "/api/v1/monitors", None))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pagination"]["total"], 6);
    assert_eq!(body["data"].as_array().unwrap().len(), 6);

    let (status, body) = app
        .request_json_anon(common::json_request(
            "GET",
            "/api/v1/reports/overview",
            None,
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["monitors"]["total"], 6);
    assert_eq!(body["monitors"]["down"], 1);
    assert_eq!(body["monitors"]["paused"], 1);

    let (status, body) = app
        .request_json_anon(common::json_request(
            "POST",
            "/api/v1/monitors",
            Some(serde_json::json!({ "url": "https://example.com" })),
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
}

#[tokio::test]
async fn demo_seed_skips_existing_data() {
    let app = TestApp::new(true).await;
    insert_monitor_raw(
        &app.state.pool,
        uuid::Uuid::new_v4(),
        "https://example.com",
        1,
    )
    .await;

    let outcome = demo_service::seed_if_empty(&app.state)
        .await
        .expect("seed attempt");
    assert_eq!(outcome, demo_service::SeedOutcome::default());

    let monitors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM monitors")
        .fetch_one(&app.state.pool)
        .await
        .expect("count monitors");
    assert_eq!(monitors, 1);
}
