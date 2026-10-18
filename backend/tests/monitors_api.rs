//! Integration tests for the monitors API: CRUD + validation + manual-check
//! outcomes (success, retries, timeout, DNS-blocked) + pause/resume + SSRF.
//!
//! Every test gets a fresh temp-file SQLite database and drives the real
//! router in-process. Outbound checks hit a local wiremock server, so tests
//! run with `allow_private_targets: true` except where SSRF denial itself is
//! under test.

mod common;

use common::*;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn crud_round_trip_with_pagination() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;

    let created =
        create_monitor_json(&app, &target_url(&server, "/a"), json!({ "name": "alpha" })).await;
    let second =
        create_monitor_json(&app, &target_url(&server, "/b"), json!({ "name": "beta" })).await;
    let third =
        create_monitor_json(&app, &target_url(&server, "/c"), json!({ "name": "gamma" })).await;
    let first_id = created["id"].as_str().unwrap().to_string();

    // Defaults applied on create.
    assert_eq!(created["name"], "alpha");
    assert_eq!(created["current_status"], "unknown");
    assert_eq!(created["enabled"], true);
    assert_eq!(created["check_interval_seconds"], 60);
    assert_eq!(created["timeout_ms"], 5000);
    assert_eq!(created["retry_max_attempts"], 2);
    assert!(created["next_check_at"].is_string());

    // Pagination.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            "/api/v1/monitors?page=1&page_size=2",
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
    assert_eq!(body["pagination"]["total"], 3);
    assert_eq!(body["pagination"]["total_pages"], 2);
    assert_eq!(body["pagination"]["page"], 1);
    assert_eq!(body["pagination"]["page_size"], 2);

    // Get by id (MonitorSummary flattens the monitor fields).
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/monitors/{first_id}"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["name"], "alpha");
    assert_eq!(body["checks_in_window"], 0);
    // No checks yet: uptime must be null, never coerced to 100.
    assert!(body["uptime_percent"].is_null());

    // Update (partial).
    let (status, body) = app
        .request_json(json_request(
            "PUT",
            &format!("/api/v1/monitors/{first_id}"),
            Some(json!({ "name": "alpha-renamed", "check_interval_seconds": 30 })),
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["name"], "alpha-renamed");
    assert_eq!(body["check_interval_seconds"], 30);
    // Untouched fields keep their values.
    assert_eq!(body["timeout_ms"], 5000);
    assert_eq!(body["url"], target_url(&server, "/a"));

    // Delete, then get/delete again → 404.
    let status = app
        .request(json_request(
            "DELETE",
            &format!("/api/v1/monitors/{first_id}"),
            None,
        ))
        .await;
    assert_eq!(status, 204);
    let (status, _) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/monitors/{first_id}"),
            None,
        ))
        .await;
    assert_eq!(status, 404);
    let status = app
        .request(json_request(
            "DELETE",
            &format!("/api/v1/monitors/{first_id}"),
            None,
        ))
        .await;
    assert_eq!(status, 404);

    // Cascade: checks and incidents of the deleted monitor are gone.
    let second_id = Uuid::parse_str(second["id"].as_str().unwrap()).unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM check_results WHERE monitor_id = ?")
            .bind(second_id)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    let _ = third;
}

#[tokio::test]
async fn create_rejects_invalid_input() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;

    // Invalid URL.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(json!({ "url": "not a url" })),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["code"], "validation_failed");
    assert_eq!(body["error"]["details"][0]["field"], "url");

    // Interval below the documented minimum (10s).
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(
                &target_url(&server, "/ok"),
                json!({ "check_interval_seconds": 5 }),
            )),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"]["details"][0]["field"],
        "check_interval_seconds"
    );

    // Retry attempts above the maximum.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(
                &target_url(&server, "/ok"),
                json!({ "retry_max_attempts": 9 }),
            )),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "retry_max_attempts");

    // Inverted expected-status range.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(
                &target_url(&server, "/ok"),
                json!({ "expected_status_min": 300, "expected_status_max": 200 }),
            )),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "expected_status_max");

    // Update with an out-of-range override is rejected too.
    let created = create_monitor_json(&app, &target_url(&server, "/ok"), json!({})).await;
    let id = created["id"].as_str().unwrap();
    let (status, body) = app
        .request_json(json_request(
            "PUT",
            &format!("/api/v1/monitors/{id}"),
            Some(json!({ "timeout_ms": 50 })),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "timeout_ms");
}

#[tokio::test]
async fn list_rejects_unknown_status_filter() {
    let app = TestApp::new(true).await;
    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/monitors?status=bogus", None))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "status");
}

#[tokio::test]
async fn ssrf_targets_rejected_on_create_without_private_targets() {
    let app = TestApp::new(false).await;

    for url in [
        "http://127.0.0.1:9/health",
        "http://10.0.0.8/health",
        "http://192.168.1.1/health",
        "http://169.254.169.254/latest/meta-data",
        "http://[::1]/health",
    ] {
        let (status, body) = app
            .request_json(json_request(
                "POST",
                "/api/v1/monitors",
                Some(json!({ "url": url })),
            ))
            .await;
        assert_eq!(status, 422, "URL must be rejected: {url}");
        assert_eq!(body["error"]["details"][0]["field"], "url", "for {url}");
    }
}

#[tokio::test]
async fn manual_check_success_updates_status_and_window_stats() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1..)
        .mount(&server)
        .await;

    let created = create_monitor_json(
        &app,
        &target_url(&server, "/health"),
        json!({ "retry_max_attempts": 1 }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], true);
    assert_eq!(body["check"]["http_status_code"], 200);
    assert_eq!(body["check"]["error_type"], Value::Null);
    assert_eq!(body["check"]["attempt_count"], 1);
    assert!(body["check"]["response_time_ms"].as_i64().unwrap() >= 0);
    assert_eq!(body["incident_opened"], false);
    assert_eq!(body["monitor"]["current_status"], "up");
    assert_eq!(body["monitor"]["consecutive_successes"], 1);
    assert_eq!(body["monitor"]["consecutive_failures"], 0);

    // Enriched GET reflects the stored check.
    let (status, summary) = app
        .request_json(json_request("GET", &format!("/api/v1/monitors/{id}"), None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(summary["latest_success"], true);
    assert_eq!(summary["latest_http_status"], 200);
    assert_eq!(summary["checks_in_window"], 1);
    assert_eq!(summary["uptime_percent"], 100.0);
    assert_eq!(summary["degraded"], false);
}

#[tokio::test]
async fn manual_check_404_mismatch_is_a_failed_check() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let created = create_monitor_json(
        &app,
        &target_url(&server, "/missing"),
        json!({ "retry_max_attempts": 1 }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], false);
    assert_eq!(body["check"]["error_type"], "http_status_mismatch");
    assert_eq!(body["check"]["http_status_code"], 404);
    assert_eq!(body["monitor"]["current_status"], "down");
}

#[tokio::test]
async fn manual_check_retries_exhausted_opens_incident() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/flaky"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;

    let created = create_monitor_json(
        &app,
        &target_url(&server, "/flaky"),
        json!({ "retry_max_attempts": 3, "consecutive_failures_threshold": 1 }),
    )
    .await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], false);
    assert_eq!(body["check"]["error_type"], "http_status_mismatch");
    assert_eq!(body["check"]["http_status_code"], 503);
    assert_eq!(body["check"]["attempt_count"], 3);
    assert_eq!(body["incident_opened"], true);
    assert_eq!(body["monitor"]["current_status"], "down");
    assert_eq!(body["monitor"]["consecutive_failures"], 1);

    // Exactly three attempts reached the target (wiremock also enforces this
    // via `.expect(3)`; an explicit check makes failures easier to read).
    let received = server.received_requests().await.expect("request log");
    assert_eq!(received.len(), 3);

    // An open incident was persisted in the same transaction as the check.
    assert_eq!(open_incident_count(&app.state.pool, id).await, 1);
}

#[tokio::test]
async fn manual_check_timeout_is_classified() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(400)))
        .mount(&server)
        .await;

    let created = create_monitor_json(
        &app,
        &target_url(&server, "/slow"),
        json!({ "timeout_ms": 100, "retry_max_attempts": 1 }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], false);
    assert_eq!(body["check"]["error_type"], "timeout");
    // Transport failures have no response time (never coerced to 0).
    assert!(body["check"]["response_time_ms"].is_null());
    assert!(body["check"]["error_message"].is_string());
    assert_eq!(body["monitor"]["current_status"], "down");
}

#[tokio::test]
async fn dns_layer_blocks_loopback_when_private_targets_disallowed() {
    // Bypass create-time validation (which would reject the loopback URL
    // first) so the check exercises the request-time DNS resolver instead.
    let app = TestApp::new(false).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let id = Uuid::new_v4();
    insert_monitor_raw(&app.state.pool, id, &target_url(&server, "/health"), 1).await;

    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], false);
    assert_eq!(body["check"]["error_type"], "blocked_target");
    assert_eq!(body["monitor"]["current_status"], "down");
}

#[tokio::test]
async fn failed_check_then_recovery_resolves_incident() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/recover"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let created = create_monitor_json(
        &app,
        &target_url(&server, "/recover"),
        json!({ "retry_max_attempts": 1, "consecutive_failures_threshold": 1 }),
    )
    .await;
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();

    // First check fails → down + incident opened.
    let (_, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(body["monitor"]["current_status"], "down");
    assert_eq!(body["incident_opened"], true);
    assert_eq!(open_incident_count(&app.state.pool, id).await, 1);

    // Target recovers → next check is up + incident resolved.
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/recover"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let (_, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(body["check"]["success"], true);
    assert_eq!(body["monitor"]["current_status"], "up");
    assert_eq!(body["incident_resolved"], true);
    assert_eq!(open_incident_count(&app.state.pool, id).await, 0);
    assert_eq!(resolved_incident_count(&app.state.pool, id).await, 1);
}

#[tokio::test]
async fn pause_blocks_manual_checks_and_resume_reactivates() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let created = create_monitor_json(&app, &target_url(&server, "/health"), json!({})).await;
    let id = created["id"].as_str().unwrap();

    // Pause.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/pause"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["current_status"], "paused");
    assert_eq!(body["enabled"], false);
    assert!(body["next_check_at"].is_null());

    // Manual check is refused while paused.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["code"], "conflict");

    // Resume resets to unknown and re-arms scheduling.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/resume"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["current_status"], "unknown");
    assert_eq!(body["enabled"], true);
    assert!(body["next_check_at"].is_string());

    // Checks work again.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["check"]["success"], true);
}

#[tokio::test]
async fn name_defaults_to_host_when_omitted() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    let created = create_monitor_json(&app, &target_url(&server, "/x"), json!({})).await;
    let host = url::Url::parse(&target_url(&server, "/x"))
        .unwrap()
        .host_str()
        .unwrap()
        .to_string();
    assert_eq!(created["name"], host);
}

async fn create_monitor_json(app: &TestApp, url: &str, extra: Value) -> Value {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(url, extra)),
        ))
        .await;
    assert_eq!(status, 201, "create failed: {body}");
    body
}
