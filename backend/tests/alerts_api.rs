//! Alerting integration tests: outbox enqueue, webhook delivery, retries,
//! duplicate protection, destination privacy, and email rendering.

mod common;

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use common::*;
use pulsewatch_backend::models::alert::AlertEvent;
use pulsewatch_backend::repositories::{alert_repository, incident_repository, monitor_repository};
use pulsewatch_backend::services::alert_service::{deliver_due_once, EmailSender, MAX_ATTEMPTS};
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[derive(Default)]
struct FakeEmail {
    sent: Mutex<Vec<(String, String, String)>>,
}

#[async_trait]
impl EmailSender for FakeEmail {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        self.sent.lock().expect("lock sent emails").push((
            to.to_string(),
            subject.to_string(),
            body.to_string(),
        ));
        Ok(())
    }
}

async fn create_monitor(app: &TestApp, url: &str, extra: Value) -> (Uuid, Value) {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(url, extra)),
        ))
        .await;
    assert_eq!(status, 201, "create failed: {body}");
    let id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    (id, body)
}

async fn run_check(app: &TestApp, id: Uuid) -> Value {
    let (status, body) = app
        .request_json(json_request(
            "POST",
            &format!("/api/v1/monitors/{id}/check"),
            None,
        ))
        .await;
    assert_eq!(status, 200, "check failed: {body}");
    body
}

async fn deliveries(app: &TestApp, monitor_id: Uuid) -> Vec<(String, String, String, i64)> {
    sqlx::query_as(
        "SELECT event, channel, status, attempts FROM alert_deliveries \
         WHERE monitor_id = ? ORDER BY event, channel",
    )
    .bind(monitor_id)
    .fetch_all(&app.state.pool)
    .await
    .expect("load deliveries")
}

async fn destination(app: &TestApp, monitor_id: Uuid, channel: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT destination FROM alert_deliveries WHERE monitor_id = ? AND channel = ?",
    )
    .bind(monitor_id)
    .bind(channel)
    .fetch_optional(&app.state.pool)
    .await
    .expect("load destination")
}

#[tokio::test]
async fn transition_enqueues_alerts_and_delivers_webhook() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    let hooks = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/down"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&target)
        .await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&hooks)
        .await;

    let hook_url = target_url(&hooks, "/hook");
    let (id, created) = create_monitor(
        &app,
        &target_url(&target, "/down"),
        json!({ "name": "API", "retry_max_attempts": 1, "alert_webhook_url": hook_url }),
    )
    .await;

    // The destination itself is never part of a public response.
    assert_eq!(created["alert_webhook_configured"], true);
    assert_eq!(created["alert_email_configured"], false);
    assert!(created.get("alert_webhook_url").is_none());
    assert!(created.get("alert_email").is_none());

    let opened = run_check(&app, id).await;
    assert_eq!(opened["incident_opened"], true);
    assert_eq!(
        deliveries(&app, id).await,
        vec![("opened".into(), "webhook".into(), "pending".into(), 0)]
    );
    assert_eq!(
        destination(&app, id, "webhook").await.as_deref(),
        Some(hook_url.as_str())
    );

    assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 1);
    assert_eq!(
        deliveries(&app, id).await,
        vec![("opened".into(), "webhook".into(), "delivered".into(), 0)]
    );

    // Recovery enqueues a second, distinct delivery.
    target.reset().await;
    Mock::given(method("GET"))
        .and(path("/down"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&target)
        .await;
    let resolved = run_check(&app, id).await;
    assert_eq!(resolved["incident_resolved"], true);
    assert_eq!(
        deliveries(&app, id).await,
        vec![
            ("opened".into(), "webhook".into(), "delivered".into(), 0),
            ("resolved".into(), "webhook".into(), "pending".into(), 0),
        ]
    );
    assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 1);

    let requests = hooks.received_requests().await.expect("webhook log");
    assert_eq!(requests.len(), 2);
    let payload: Value = serde_json::from_slice(requests[1].body.as_slice()).unwrap();
    assert_eq!(payload["event"], "resolved");
    assert_eq!(payload["monitor"]["name"], "API");
}

#[tokio::test]
async fn retries_until_max_attempts_then_marks_failed() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    let hooks = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&target)
        .await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(500))
        .expect(MAX_ATTEMPTS as u64)
        .mount(&hooks)
        .await;

    let (id, _) = create_monitor(
        &app,
        &target_url(&target, "/down"),
        json!({ "retry_max_attempts": 1, "alert_webhook_url": target_url(&hooks, "/hook") }),
    )
    .await;
    run_check(&app, id).await;

    for attempt in 1..=MAX_ATTEMPTS {
        sqlx::query("UPDATE alert_deliveries SET next_attempt_at = ? WHERE monitor_id = ?")
            .bind(Utc::now() - ChronoDuration::seconds(1))
            .bind(id)
            .execute(&app.state.pool)
            .await
            .unwrap();
        assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 1);
        let rows = deliveries(&app, id).await;
        assert_eq!(rows[0].3, attempt);
        let expected_status = if attempt == MAX_ATTEMPTS {
            "failed"
        } else {
            "pending"
        };
        assert_eq!(rows[0].2, expected_status);
    }

    // Failed deliveries are never picked up again.
    sqlx::query("UPDATE alert_deliveries SET next_attempt_at = ? WHERE monitor_id = ?")
        .bind(Utc::now() - ChronoDuration::seconds(1))
        .bind(id)
        .execute(&app.state.pool)
        .await
        .unwrap();
    assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 0);
}

#[tokio::test]
async fn permanent_webhook_failure_is_not_retried() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    let hooks = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&target)
        .await;
    Mock::given(method("POST"))
        .and(path("/hook"))
        .respond_with(ResponseTemplate::new(400))
        .expect(1)
        .mount(&hooks)
        .await;

    let (id, _) = create_monitor(
        &app,
        &target_url(&target, "/down"),
        json!({ "retry_max_attempts": 1, "alert_webhook_url": target_url(&hooks, "/hook") }),
    )
    .await;
    run_check(&app, id).await;

    assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 1);
    assert_eq!(
        deliveries(&app, id).await,
        vec![("opened".into(), "webhook".into(), "failed".into(), 1)]
    );
    assert_eq!(deliver_due_once(&app.state, None).await.unwrap(), 0);
}

#[tokio::test]
async fn email_delivery_uses_fake_sender_and_renders_subject_once() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&target)
        .await;

    let (id, _) = create_monitor(
        &app,
        &target_url(&target, "/down"),
        json!({
            "name": "Billing API",
            "retry_max_attempts": 1,
            "alert_email": "oncall@example.com",
        }),
    )
    .await;
    run_check(&app, id).await;

    let email = FakeEmail::default();
    assert_eq!(deliver_due_once(&app.state, Some(&email)).await.unwrap(), 1);
    {
        let sent = email.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "oncall@example.com");
        assert_eq!(sent[0].1, "[PulseWatch] DOWN: Billing API");
        assert!(sent[0].2.contains("Billing API"));
        assert!(sent[0].2.contains(&target_url(&target, "/down")));
    }
    assert_eq!(
        deliveries(&app, id).await,
        vec![("opened".into(), "email".into(), "delivered".into(), 0)]
    );
}

#[tokio::test]
async fn duplicate_transition_enqueue_is_ignored() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&target)
        .await;

    let (id, _) = create_monitor(
        &app,
        &target_url(&target, "/down"),
        json!({ "retry_max_attempts": 1, "alert_webhook_url": "https://hooks.example.com/x" }),
    )
    .await;
    run_check(&app, id).await;

    let mut tx = app.state.pool.begin().await.unwrap();
    let monitor = monitor_repository::find_by_id(&mut *tx, id)
        .await
        .unwrap()
        .unwrap();
    let incident = incident_repository::find_open(&mut *tx, id)
        .await
        .unwrap()
        .unwrap();
    alert_repository::enqueue_for_transition(
        &mut tx,
        &monitor,
        &incident,
        AlertEvent::Opened,
        Utc::now(),
    )
    .await
    .unwrap();
    alert_repository::enqueue_for_transition(
        &mut tx,
        &monitor,
        &incident,
        AlertEvent::Opened,
        Utc::now(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(deliveries(&app, id).await.len(), 1);
}

#[tokio::test]
async fn alert_destinations_are_write_only_and_empty_clears() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    let hook = "https://hooks.example.com/alerts";

    let (id, created) = create_monitor(
        &app,
        &target_url(&target, "/ok"),
        json!({ "alert_webhook_url": hook, "alert_email": "ops@example.com" }),
    )
    .await;
    assert_eq!(created["alert_webhook_configured"], true);
    assert_eq!(created["alert_email_configured"], true);

    // Omitted fields keep their destinations.
    let (status, updated) = app
        .request_json(json_request(
            "PUT",
            &format!("/api/v1/monitors/{id}"),
            Some(json!({ "name": "renamed" })),
        ))
        .await;
    assert_eq!(status, 200, "{updated}");
    assert_eq!(updated["alert_webhook_configured"], true);
    assert_eq!(updated["alert_email_configured"], true);

    // Explicit empty strings clear them.
    let (status, cleared) = app
        .request_json(json_request(
            "PUT",
            &format!("/api/v1/monitors/{id}"),
            Some(json!({ "alert_webhook_url": "", "alert_email": "" })),
        ))
        .await;
    assert_eq!(status, 200, "{cleared}");
    assert_eq!(cleared["alert_webhook_configured"], false);
    assert_eq!(cleared["alert_email_configured"], false);

    let row: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT alert_webhook_url, alert_email FROM monitors WHERE id = ?")
            .bind(id)
            .fetch_one(&app.state.pool)
            .await
            .unwrap();
    assert_eq!(row, (None, None));
}

#[tokio::test]
async fn openapi_schema_exposes_configuration_flags_not_destinations() {
    let app = TestApp::new(true).await;
    let (status, spec) = app
        .request_json(json_request("GET", "/api/v1/openapi.json", None))
        .await;
    assert_eq!(status, 200);
    let properties = &spec["components"]["schemas"]["Monitor"]["properties"];
    assert_eq!(properties["alert_webhook_configured"]["type"], "boolean");
    assert_eq!(properties["alert_email_configured"]["type"], "boolean");
    assert!(properties.get("alert_webhook_url").is_none());
    assert!(properties.get("alert_email").is_none());
}

#[tokio::test]
async fn invalid_destinations_are_rejected() {
    let app = TestApp::new(true).await;
    let target = MockServer::start().await;
    let url = target_url(&target, "/ok");

    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(&url, json!({ "alert_email": "not-an-email" }))),
        ))
        .await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["error"]["details"][0]["field"], "alert_email");

    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(
                &url,
                json!({ "alert_webhook_url": "ftp://example.com/hook" }),
            )),
        ))
        .await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["error"]["details"][0]["field"], "alert_webhook_url");
}

#[tokio::test]
async fn private_webhook_is_rejected_when_private_targets_are_disallowed() {
    let app = TestApp::new(false).await;

    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body(
                "https://example.com/health",
                json!({ "alert_webhook_url": "http://127.0.0.1:9999/hook" }),
            )),
        ))
        .await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["error"]["details"][0]["field"], "alert_webhook_url");
}
