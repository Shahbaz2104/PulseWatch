//! Scheduler integration tests: restart recovery (overdue monitor checked
//! once), no-claim for future/paused monitors, overlap prevention against a
//! concurrent manual check, and clean shutdown of the loop.
//!
//! `run_due_once` is called directly (instead of sleeping on the 1s loop)
//! and its returned task handles are awaited for determinism.

mod common;

use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use common::*;
use serde_json::json;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use pulsewatch_backend::services::scheduler;

async fn set_next_check_at(app: &TestApp, id: Uuid, when: chrono::DateTime<Utc>) {
    sqlx::query("UPDATE monitors SET next_check_at = ? WHERE id = ?")
        .bind(when)
        .bind(id)
        .execute(&app.state.pool)
        .await
        .expect("update next_check_at");
}

async fn check_count(app: &TestApp, id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM check_results WHERE monitor_id = ?")
        .bind(id)
        .fetch_one(&app.state.pool)
        .await
        .expect("count checks")
}

async fn create_monitor(app: &TestApp, url: &str) -> Uuid {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(url, json!({ "retry_max_attempts": 1 }))),
        ))
        .await;
    assert_eq!(status, 201, "create failed: {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn overdue_monitor_is_checked_exactly_once_after_restart() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let id = create_monitor(&app, &target_url(&server, "/health")).await;
    // Simulate downtime: the monitor has been due for five minutes.
    set_next_check_at(&app, id, Utc::now() - ChronoDuration::minutes(5)).await;

    let handles = scheduler::run_due_once(&app.state).await;
    assert_eq!(handles.len(), 1, "overdue monitor must be claimed once");
    for handle in handles {
        handle.await.expect("check task");
    }

    // Exactly one HTTP request and one persisted check — no catch-up burst.
    let received = server.received_requests().await.expect("request log");
    assert_eq!(received.len(), 1);
    assert_eq!(check_count(&app, id).await, 1);

    // The check advanced the schedule into the future.
    let (status, summary) = app
        .request_json(json_request("GET", &format!("/api/v1/monitors/{id}"), None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(summary["latest_success"], true);
    assert_eq!(summary["current_status"], "up");
    let next = summary["next_check_at"].as_str().expect("next_check_at");
    let next: chrono::DateTime<Utc> = next.parse().unwrap();
    assert!(next > Utc::now(), "next_check_at must be in the future");

    // A second scan finds nothing to do.
    let handles = scheduler::run_due_once(&app.state).await;
    assert!(handles.is_empty(), "no second claim for the same interval");
}

#[tokio::test]
async fn future_and_paused_monitors_are_not_claimed() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    // Due far in the future.
    let future_id = create_monitor(&app, &target_url(&server, "/future")).await;
    set_next_check_at(&app, future_id, Utc::now() + ChronoDuration::hours(1)).await;

    // Paused (enabled = 0) but still carrying a past due time.
    let paused_id = create_monitor(&app, &target_url(&server, "/paused")).await;
    let (status, _) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{paused_id}/pause"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    set_next_check_at(&app, paused_id, Utc::now() - ChronoDuration::minutes(5)).await;

    let handles = scheduler::run_due_once(&app.state).await;
    assert!(handles.is_empty(), "nothing due must be claimed");
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn scheduled_and_manual_checks_never_overlap() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    // Slow response keeps the scheduled check in flight long enough for the
    // manual attempt to race it.
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(400)))
        .mount(&server)
        .await;

    let id = create_monitor(&app, &target_url(&server, "/slow")).await;
    set_next_check_at(&app, id, Utc::now()).await;

    // Dispatch the scheduled check but do not await it yet.
    let handles = scheduler::run_due_once(&app.state).await;
    assert_eq!(handles.len(), 1);

    // Wait until the scheduled check holds the in-flight slot.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !app.state.in_flight.contains(id).await {
        assert!(
            Instant::now() < deadline,
            "scheduled check never went in flight"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // The manual check is refused instead of running a second probe.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["code"], "conflict");

    for handle in handles {
        handle.await.expect("check task");
    }

    // Exactly one probe reached the target and one check row was written.
    let received = server.received_requests().await.expect("request log");
    assert_eq!(received.len(), 1);
    assert_eq!(check_count(&app, id).await, 1);

    // The slot is released: a later manual check succeeds.
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
async fn scheduler_loop_stops_on_shutdown() {
    let app = TestApp::new(true).await;
    let handle = scheduler::spawn(app.state.clone());
    // Give the loop a moment to start, then cancel.
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.state.shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("scheduler must stop promptly after shutdown")
        .expect("scheduler task");
}
