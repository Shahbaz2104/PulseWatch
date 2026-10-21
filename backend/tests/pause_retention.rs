//! Integration tests for pause intervals, paused-time reporting, downtime
//! exclusion, and retention cleanup.

mod common;

use chrono::{DateTime, Duration, SecondsFormat, Timelike, Utc};
use common::*;
use pulsewatch_backend::models::check_result::{CheckErrorType, CheckResult};
use pulsewatch_backend::repositories::check_repository;
use pulsewatch_backend::services::monitoring_service::{self, CheckTrigger};
use pulsewatch_backend::services::retention_service;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

async fn create_monitor(app: &TestApp, name: &str) -> Uuid {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(json!({
                "name": name,
                "url": format!("http://example.com/{name}"),
            })),
        ))
        .await;
    assert_eq!(status, 201, "create failed: {body}");
    Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
}

async fn seed_incident(
    app: &TestApp,
    monitor_id: Uuid,
    started_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
) -> Uuid {
    let id = Uuid::new_v4();
    let status = if resolved_at.is_some() {
        "resolved"
    } else {
        "open"
    };
    sqlx::query(
        "INSERT INTO incidents (id, monitor_id, started_at, resolved_at, status, \
         failure_reason, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'outage', ?, ?)",
    )
    .bind(id)
    .bind(monitor_id)
    .bind(started_at)
    .bind(resolved_at)
    .bind(status)
    .bind(started_at)
    .bind(started_at)
    .execute(&app.state.pool)
    .await
    .expect("seed incident");
    id
}

async fn seed_pause(
    app: &TestApp,
    monitor_id: Uuid,
    paused_at: DateTime<Utc>,
    resumed_at: Option<DateTime<Utc>>,
) {
    sqlx::query(
        "INSERT INTO monitor_pauses (id, monitor_id, paused_at, resumed_at) VALUES (?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_id)
    .bind(paused_at)
    .bind(resumed_at)
    .execute(&app.state.pool)
    .await
    .expect("seed pause");
}

async fn seed_check(app: &TestApp, monitor_id: Uuid, checked_at: DateTime<Utc>) {
    let check = CheckResult {
        id: Uuid::new_v4(),
        monitor_id,
        checked_at,
        completed_at: Some(checked_at),
        http_status_code: Some(200),
        response_time_ms: Some(20),
        success: true,
        error_type: None::<CheckErrorType>,
        error_message: None,
        attempt_count: 1,
        expected_status_match: true,
    };
    check_repository::insert(&app.state.pool, &check)
        .await
        .expect("seed check");
}

fn window_query(from: DateTime<Utc>, to: DateTime<Utc>) -> String {
    format!(
        "window=custom&from={}&to={}",
        from.to_rfc3339_opts(SecondsFormat::Secs, true),
        to.to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

async fn pause(app: &TestApp, id: Uuid) -> Value {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/pause"),
            None,
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    body
}

async fn resume(app: &TestApp, id: Uuid) -> Value {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/resume"),
            None,
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    body
}

#[tokio::test]
async fn pause_opens_one_interval_and_resolves_incident_without_alert() {
    let app = TestApp::new(true).await;
    let monitor_id = create_monitor(&app, "pausable").await;
    let incident_id =
        seed_incident(&app, monitor_id, Utc::now() - Duration::minutes(10), None).await;

    let body = pause(&app, monitor_id).await;
    assert_eq!(body["current_status"], "paused");
    assert_eq!(body["enabled"], false);
    assert!(body["next_check_at"].is_null());

    assert_eq!(open_incident_count(&app.state.pool, monitor_id).await, 0);
    assert_eq!(
        resolved_incident_count(&app.state.pool, monitor_id).await,
        1
    );
    let deliveries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM alert_deliveries WHERE incident_id = ?")
            .bind(incident_id)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(deliveries, 0, "pause must not enqueue a recovery alert");

    // Pausing again is idempotent: one open interval, not two.
    pause(&app, monitor_id).await;
    let open_intervals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM monitor_pauses WHERE monitor_id = ? AND resumed_at IS NULL",
    )
    .bind(monitor_id)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert_eq!(open_intervals, 1);

    // Manual checks remain blocked while paused (no network call is made).
    let status = app
        .request(json_request(
            "POST",
            &format!("/api/v1/monitors/{monitor_id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 409);

    let body = resume(&app, monitor_id).await;
    assert_eq!(body["current_status"], "unknown");
    assert_eq!(body["enabled"], true);
    assert!(body["next_check_at"].is_string());

    let resumed_at: String = sqlx::query_scalar(
        "SELECT resumed_at FROM monitor_pauses WHERE monitor_id = ? AND resumed_at IS NOT NULL",
    )
    .bind(monitor_id)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert!(!resumed_at.is_empty());

    // Resume is idempotent and does not rewrite the closed interval.
    resume(&app, monitor_id).await;
    let unchanged: String = sqlx::query_scalar(
        "SELECT resumed_at FROM monitor_pauses WHERE monitor_id = ? AND resumed_at IS NOT NULL",
    )
    .bind(monitor_id)
    .fetch_one(&app.state.pool)
    .await
    .unwrap();
    assert_eq!(resumed_at, unchanged);
    let intervals: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM monitor_pauses WHERE monitor_id = ?")
            .bind(monitor_id)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(intervals, 1);
}

#[tokio::test]
async fn reports_clip_paused_seconds_to_the_window() {
    let app = TestApp::new(true).await;
    let monitor_id = create_monitor(&app, "paused-report").await;

    let to = (Utc::now() - Duration::hours(1))
        .with_nanosecond(0)
        .unwrap();
    let from = to - Duration::hours(10);
    // Starts before the window: only one hour overlaps.
    seed_pause(
        &app,
        monitor_id,
        from - Duration::hours(1),
        Some(from + Duration::hours(1)),
    )
    .await;
    // Open pause inside the window: clipped at the window end.
    seed_pause(&app, monitor_id, to - Duration::minutes(30), None).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/reports/uptime?{}", window_query(from, to)),
            None,
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["total_paused_seconds"], 5_400);
    let row = &body["monitors"].as_array().unwrap()[0];
    assert_eq!(row["paused_seconds"], 5_400);
    assert!(row["uptime_percent"].is_null());
}

#[tokio::test]
async fn downtime_subtracts_paused_overlap() {
    let app = TestApp::new(true).await;
    let monitor_id = create_monitor(&app, "downtime-paused").await;

    let to = Utc::now().with_nanosecond(0).unwrap();
    let from = to - Duration::hours(10);
    let incident_start = from + Duration::hours(2);
    let incident_end = from + Duration::hours(6);
    seed_incident(&app, monitor_id, incident_start, Some(incident_end)).await;
    seed_pause(
        &app,
        monitor_id,
        incident_start + Duration::hours(1),
        Some(incident_start + Duration::hours(3)),
    )
    .await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/reports/incidents?{}", window_query(from, to)),
            None,
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["stats"]["outage_count"], 1);
    assert_eq!(
        body["stats"]["downtime_seconds"], 7_200,
        "4h incident minus 2h paused"
    );
}

#[tokio::test]
async fn retention_purges_old_checks_and_completed_deliveries_only() {
    let app = TestApp::new(true).await;
    let monitor_id = create_monitor(&app, "retained").await;
    let now = Utc::now();
    let incident_id = seed_incident(&app, monitor_id, now - Duration::days(120), None).await;

    seed_check(&app, monitor_id, now - Duration::days(100)).await;
    seed_check(&app, monitor_id, now - Duration::days(1)).await;

    for (event, channel, status, created, delivered) in [
        (
            "opened",
            "webhook",
            "delivered",
            now - Duration::days(100),
            Some(now - Duration::days(100)),
        ),
        (
            "resolved",
            "webhook",
            "failed",
            now - Duration::days(100),
            None,
        ),
        (
            "opened",
            "email",
            "pending",
            now - Duration::days(100),
            None,
        ),
        (
            "resolved",
            "email",
            "delivered",
            now - Duration::days(1),
            Some(now - Duration::days(1)),
        ),
    ] {
        sqlx::query(
            "INSERT INTO alert_deliveries (id, monitor_id, incident_id, event, channel, \
             destination, payload, status, attempts, next_attempt_at, created_at, delivered_at) \
             VALUES (?, ?, ?, ?, ?, 'https://example.com/hook', '{}', ?, 1, ?, ?, ?)",
        )
        .bind(Uuid::new_v4())
        .bind(monitor_id)
        .bind(incident_id)
        .bind(event)
        .bind(channel)
        .bind(status)
        .bind(created)
        .bind(created)
        .bind(delivered)
        .execute(&app.state.pool)
        .await
        .expect("seed delivery");
    }

    let stats = retention_service::purge_once(&app.state)
        .await
        .expect("purge");
    assert_eq!(stats.checks, 1);
    assert_eq!(stats.alert_deliveries, 2);

    let checks: Vec<DateTime<Utc>> = sqlx::query_scalar("SELECT checked_at FROM check_results")
        .fetch_all(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(checks.len(), 1);
    assert!(checks[0] > now - Duration::days(2), "fresh check retained");

    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM alert_deliveries ORDER BY status")
            .fetch_all(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(statuses, vec!["delivered", "pending"]);

    let incidents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM incidents")
        .fetch_one(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(incidents, 1, "incidents are never deleted");
}

#[tokio::test]
async fn in_flight_check_cannot_reopen_incident_after_pause() {
    use std::time::Duration as StdDuration;

    let app = TestApp::new(true).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(StdDuration::from_millis(300)))
        .mount(&server)
        .await;

    let created = create_monitor(&app, "race").await;
    let (_, body) = app
        .request_json(json_request(
            "PUT",
            &format!("/api/v1/monitors/{created}"),
            Some(json!({ "url": target_url(&server, "/slow") })),
        ))
        .await;
    assert_eq!(body["id"], created.to_string());

    let state = app.state.clone();
    let check = tokio::spawn(async move {
        monitoring_service::run_check(&state, created, CheckTrigger::Manual).await
    });

    // Wait until the outbound check owns the in-flight slot, then pause while
    // the response is still delayed.
    tokio::time::timeout(StdDuration::from_secs(2), async {
        while !app.state.in_flight.contains(created).await {
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .expect("check should start");

    let body = pause(&app, created).await;
    assert_eq!(body["current_status"], "paused");

    let outcome = check.await.expect("join").expect("check outcome");
    assert!(outcome.result.success);
    assert_eq!(outcome.monitor.current_status.as_str(), "paused");
    assert!(!outcome.incident_opened);
    assert_eq!(open_incident_count(&app.state.pool, created).await, 0);
}
