//! Integration tests for the reports API with hand-computed metric values.
//!
//! Scenario (window = custom [now-3h, now)):
//! - Monitor A ("alpha"): 11 seeded checks — 10 inside the window
//!   (2 hour-aligned buckets: 8 successes 10..=45 ms + 2 failures with
//!   response times 999 ms that must be excluded from latency stats) plus
//!   1 old check outside the window that must be ignored everywhere.
//! - Monitor B ("beta"): no checks at all — every metric must be null.
//! - 2 incidents on A: one resolved (exactly 1800 s) and one open.
//!
//! Expected values (success response times: early 10,20,30,40; late 15,25,35,45):
//! - uptime A = 8/10 = 80.0%; overall = 80.0%; B = null.
//! - latency A: samples 8, avg 220/8 = 27.5, median (rank ceil(4)=4 →
//!   offset 3) = 25, p95 (rank ceil(7.6)=8 → offset 7) = 45.

mod common;

use chrono::{DateTime, Duration, SecondsFormat, Timelike, Utc};
use common::*;
use pulsewatch_backend::models::check_result::{CheckErrorType, CheckResult};
use pulsewatch_backend::repositories::check_repository;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::MockServer;

struct Seeded {
    monitor_a: Uuid,
    monitor_b: Uuid,
    from: chrono::DateTime<Utc>,
    to: chrono::DateTime<Utc>,
}

async fn seed_check(
    app: &TestApp,
    monitor_id: Uuid,
    checked_at: chrono::DateTime<Utc>,
    success: bool,
    http_status: Option<i64>,
    response_time_ms: Option<i64>,
) {
    let check = CheckResult {
        id: Uuid::new_v4(),
        monitor_id,
        checked_at,
        completed_at: Some(checked_at + Duration::milliseconds(50)),
        http_status_code: http_status,
        response_time_ms,
        success,
        error_type: if success {
            None
        } else {
            Some(CheckErrorType::HttpStatusMismatch)
        },
        error_message: if success {
            None
        } else {
            Some("unexpected HTTP status".to_string())
        },
        attempt_count: 1,
        expected_status_match: success,
    };
    check_repository::insert(&app.state.pool, &check)
        .await
        .expect("seed check");
}

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

async fn seed_scenario(app: &TestApp) -> Seeded {
    let monitor_a = create_monitor(app, "alpha").await;
    let monitor_b = create_monitor(app, "beta").await;

    let to = Utc::now();
    let from = to - Duration::hours(3);
    // Hour-aligned buckets (two completed past hours) so every check falls
    // strictly inside one hourly bucket regardless of the current minute.
    let hour_start = to
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let early = hour_start - Duration::hours(2);
    let late = hour_start - Duration::hours(1);

    // 4 successes + 1 failure per hour, strictly inside that hour.
    for (base, offset) in [(early, 0), (late, 5)] {
        for i in 0..4 {
            let success_at = base + Duration::minutes(5 + i);
            seed_check(
                app,
                monitor_a,
                success_at,
                true,
                Some(200),
                Some(10 * (i + 1) + offset),
            )
            .await;
        }
        seed_check(
            app,
            monitor_a,
            base + Duration::minutes(10),
            false,
            Some(503),
            Some(999), // must never leak into latency stats
        )
        .await;
    }

    // One check older than the window: excluded from every report.
    seed_check(
        app,
        monitor_a,
        from - Duration::hours(1),
        true,
        Some(200),
        Some(1),
    )
    .await;

    // Resolved incident: exactly 1800 s. Open incident: measured at query time.
    let resolved_start = to - Duration::minutes(90);
    sqlx::query(
        "INSERT INTO incidents (id, monitor_id, started_at, resolved_at, status, \
         failure_reason, created_at, updated_at) VALUES (?, ?, ?, ?, 'resolved', 'outage', ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_a)
    .bind(resolved_start)
    .bind(resolved_start + Duration::minutes(30))
    .bind(resolved_start)
    .bind(resolved_start)
    .execute(&app.state.pool)
    .await
    .expect("seed resolved incident");

    let open_start = to - Duration::minutes(45);
    sqlx::query(
        "INSERT INTO incidents (id, monitor_id, started_at, resolved_at, status, \
         failure_reason, created_at, updated_at) VALUES (?, ?, ?, NULL, 'open', 'outage', ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_a)
    .bind(open_start)
    .bind(open_start)
    .bind(open_start)
    .execute(&app.state.pool)
    .await
    .expect("seed open incident");

    // Census statuses for the overview.
    sqlx::query("UPDATE monitors SET current_status = 'up' WHERE id = ?")
        .bind(monitor_a)
        .execute(&app.state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE monitors SET current_status = 'down' WHERE id = ?")
        .bind(monitor_b)
        .execute(&app.state.pool)
        .await
        .unwrap();

    Seeded {
        monitor_a,
        monitor_b,
        from,
        to,
    }
}

fn window_query(from: DateTime<Utc>, to: DateTime<Utc>) -> String {
    // Z-suffix avoids '+' in the query string (which decodes as a space).
    format!(
        "window=custom&from={}&to={}",
        from.to_rfc3339_opts(SecondsFormat::Secs, true),
        to.to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

#[tokio::test]
async fn checks_summary_matches_hand_computed_metrics() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/monitors/{}/checks/summary?{}",
                seed.monitor_a,
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["total_checks"], 10);
    assert_eq!(body["successful_checks"], 8);
    assert_eq!(body["uptime_percent"], 80.0);
    assert_eq!(body["avg_response_time_ms"], 27.5);
    assert_eq!(body["median_response_time_ms"], 25.0);
    assert_eq!(body["p95_response_time_ms"], 45.0);
    // Newest check overall is the late-group failure with response time 999.
    assert_eq!(body["latest_response_time_ms"], 999);
}

#[tokio::test]
async fn summary_nulls_when_monitor_has_no_checks() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/monitors/{}/checks/summary?{}",
                seed.monitor_b,
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["total_checks"], 0);
    assert!(body["uptime_percent"].is_null(), "never fabricate 100%");
    assert!(body["avg_response_time_ms"].is_null());
    assert!(body["median_response_time_ms"].is_null());
    assert!(body["p95_response_time_ms"].is_null());
    assert!(body["latest_response_time_ms"].is_null());
}

#[tokio::test]
async fn checks_history_paginates_and_filters() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;
    let query = window_query(seed.from, seed.to);

    // Page 1 of the full window: 3 per page, newest first.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/monitors/{}/checks?{query}&page=1&page_size=3",
                seed.monitor_a
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
    assert_eq!(body["pagination"]["total"], 10);
    assert_eq!(body["pagination"]["total_pages"], 4);
    let first_ts = data[0]["checked_at"].as_str().unwrap().to_string();
    let third_ts = data[2]["checked_at"].as_str().unwrap().to_string();
    assert!(first_ts > third_ts, "newest first");

    // Failure filter: exactly the 2 seeded failures (no 999 latency leak in
    // the latency endpoints is asserted elsewhere).
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/monitors/{}/checks?{query}&result=failure",
                seed.monitor_a
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["pagination"]["total"], 2);
    for row in body["data"].as_array().unwrap() {
        assert_eq!(row["success"], false);
        assert_eq!(row["error_type"], "http_status_mismatch");
    }

    // Invalid result filter → 422.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/monitors/{}/checks?result=maybe", seed.monitor_a),
            None,
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "result");
}

#[tokio::test]
async fn checks_latest_returns_newest_row() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/monitors/{}/checks/latest", seed.monitor_a),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    // Newest seeded check is the late-group failure (rt 999).
    assert_eq!(body["response_time_ms"], 999);
    assert_eq!(body["success"], false);

    // Monitor B has no checks → 404 with a clear message.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/monitors/{}/checks/latest", seed.monitor_b),
            None,
        ))
        .await;
    assert_eq!(status, 404);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("no checks"));
}

#[tokio::test]
async fn unknown_monitor_returns_404_everywhere() {
    let app = TestApp::new(true).await;
    let missing = Uuid::new_v4();
    for path in [
        format!("/api/v1/monitors/{missing}/checks"),
        format!("/api/v1/monitors/{missing}/checks/latest"),
        format!("/api/v1/monitors/{missing}/checks/summary"),
    ] {
        let (status, _) = app.request_json(json_request("GET", &path, None)).await;
        assert_eq!(status, 404, "for {path}");
    }
}

#[tokio::test]
async fn uptime_report_matches_hand_computed_values() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/uptime?{}",
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["total_checks"], 10);
    assert_eq!(body["total_successes"], 8);
    assert_eq!(body["overall_uptime_percent"], 80.0);

    let monitors = body["monitors"].as_array().unwrap();
    assert_eq!(monitors.len(), 2);
    let alpha = &monitors[0]; // ordered by name
    let beta = &monitors[1];
    assert_eq!(alpha["name"], "alpha");
    assert_eq!(alpha["uptime_percent"], 80.0);
    assert_eq!(alpha["checks"], 10);
    assert_eq!(alpha["current_status"], "up");
    assert_eq!(beta["name"], "beta");
    assert!(beta["uptime_percent"].is_null());
    assert_eq!(beta["checks"], 0);
}

#[tokio::test]
async fn response_times_report_percentiles_and_series() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/response-times?{}",
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["bucket"], "hour");

    let monitors = body["monitors"].as_array().unwrap();
    let alpha = &monitors[0];
    assert_eq!(alpha["samples"], 8);
    assert_eq!(alpha["avg_response_time_ms"], 27.5);
    assert_eq!(alpha["median_response_time_ms"], 25.0);
    assert_eq!(alpha["p95_response_time_ms"], 45.0);
    let beta = &monitors[1];
    assert_eq!(beta["samples"], 0);
    assert!(beta["avg_response_time_ms"].is_null());
    assert!(beta["median_response_time_ms"].is_null());

    // Two hourly buckets, 5 checks each, 4 successes each → 80% per bucket.
    let series = body["series"].as_array().unwrap();
    assert_eq!(series.len(), 2, "exactly two hourly buckets: {series:?}");
    for bucket in series {
        assert_eq!(bucket["checks"], 5);
        assert_eq!(bucket["successes"], 4);
        assert_eq!(bucket["uptime_percent"], 80.0);
        // avg over the bucket's successes only (failures' 999 excluded).
        assert!(bucket["avg_response_time_ms"].as_f64().is_some());
    }

    // Scoped to monitor B: empty series, zero samples.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/response-times?{}&monitor_id={}",
                window_query(seed.from, seed.to),
                seed.monitor_b
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["monitors"].as_array().unwrap().len(), 1);
    assert!(body["series"].as_array().unwrap().is_empty());

    // Unknown monitor scope → 404.
    let (status, _) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/response-times?{}&monitor_id={}",
                window_query(seed.from, seed.to),
                Uuid::new_v4()
            ),
            None,
        ))
        .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn overview_reports_census_uptime_and_incidents() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/overview?{}",
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);

    assert_eq!(body["monitors"]["total"], 2);
    assert_eq!(body["monitors"]["up"], 1);
    assert_eq!(body["monitors"]["down"], 1);
    assert_eq!(body["monitors"]["unknown"], 0);
    assert_eq!(body["monitors"]["paused"], 0);

    assert_eq!(body["uptime_percent"], 80.0);
    assert_eq!(body["avg_response_time_ms"], 27.5);

    assert_eq!(body["incidents"]["outage_count"], 2);
    // resolved 1800 + open (~2700, time-dependent).
    let downtime = body["incidents"]["downtime_seconds"].as_i64().unwrap();
    assert!(
        (4490..=4510).contains(&downtime),
        "unexpected downtime {downtime}"
    );

    assert_eq!(body["recent_incidents"].as_array().unwrap().len(), 2);
    assert_eq!(body["recent_checks"].as_array().unwrap().len(), 10);

    // The 24h sparklines include the old check outside the query window:
    // two seeded buckets (5 checks, 4 successes, 80%) plus one bucket with
    // just the old successful check (1 check, 100%).
    for key in ["uptime_series", "response_time_series"] {
        let series = body[key].as_array().unwrap();
        assert_eq!(series.len(), 3, "for {key}: {series:?}");
        let total_checks: i64 = series.iter().map(|b| b["checks"].as_i64().unwrap()).sum();
        assert_eq!(total_checks, 11, "for {key}");
        let seeded: Vec<_> = series.iter().filter(|b| b["checks"] == 5).collect();
        assert_eq!(seeded.len(), 2, "for {key}");
        for bucket in seeded {
            assert_eq!(bucket["successes"], 4);
            assert_eq!(bucket["uptime_percent"], 80.0);
        }
        let old: Vec<_> = series.iter().filter(|b| b["checks"] == 1).collect();
        assert_eq!(old.len(), 1, "for {key}");
        assert_eq!(old[0]["uptime_percent"], 100.0);
    }
}

#[tokio::test]
async fn incident_report_and_incident_endpoints() {
    let app = TestApp::new(true).await;
    let seed = seed_scenario(&app).await;

    // Windowed incident report.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            &format!(
                "/api/v1/reports/incidents?{}",
                window_query(seed.from, seed.to)
            ),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["stats"]["outage_count"], 2);
    let incidents = body["incidents"].as_array().unwrap();
    assert_eq!(incidents.len(), 2);
    // Newest first: open incident (started -45m) before resolved (-90m).
    // IncidentWithMonitor flattens the incident fields.
    assert_eq!(incidents[0]["status"], "open");
    assert_eq!(incidents[0]["monitor_name"], "alpha");
    assert!(incidents[0]["duration_seconds"].as_i64().unwrap() >= 2690);
    assert_eq!(incidents[1]["status"], "resolved");
    assert_eq!(incidents[1]["duration_seconds"], 1800);

    // Incident list with status filter.
    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/incidents?status=open", None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["pagination"]["total"], 1);
    let open_id = body["data"][0]["id"].as_str().unwrap();

    // Detail fetch + duration consistency (IncidentWithMonitor is flattened).
    let (status, detail) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/incidents/{open_id}"),
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(detail["status"], "open");
    assert!(detail["duration_seconds"].as_i64().unwrap() >= 2690);
    assert_eq!(detail["monitor_url"], "http://example.com/alpha");

    // Resolved filter + 404 on unknown id + invalid status filter.
    let (status, body) = app
        .request_json(json_request(
            "GET",
            "/api/v1/incidents?status=resolved",
            None,
        ))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["pagination"]["total"], 1);

    let (status, _) = app
        .request_json(json_request(
            "GET",
            &format!("/api/v1/incidents/{}", Uuid::new_v4()),
            None,
        ))
        .await;
    assert_eq!(status, 404);

    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/incidents?status=bogus", None))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["details"][0]["field"], "status");
}

#[tokio::test]
async fn invalid_report_windows_are_rejected() {
    let app = TestApp::new(true).await;
    let server = MockServer::start().await; // unused; keeps parity with other suites
    drop(server);

    for query in [
        "window=bogus",
        "window=custom", // missing from
        "window=custom&from=not-a-date",
    ] {
        let (status, body) = app
            .request_json(json_request(
                "GET",
                &format!("/api/v1/reports/uptime?{query}"),
                None,
            ))
            .await;
        assert_eq!(status, 422, "for {query}: {body}");
        assert_eq!(body["error"]["code"], "validation_failed");
    }
}

#[tokio::test]
async fn empty_installation_reports_nulls_not_fabricated_values() {
    let app = TestApp::new(true).await;

    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/reports/overview", None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["monitors"]["total"], 0);
    assert!(body["uptime_percent"].is_null());
    assert!(body["avg_response_time_ms"].is_null());
    assert_eq!(body["incidents"]["outage_count"], 0);
    assert_eq!(body["incidents"]["downtime_seconds"], 0);
    assert!(body["recent_incidents"].as_array().unwrap().is_empty());
    assert!(body["recent_checks"].as_array().unwrap().is_empty());
    assert!(body["uptime_series"].as_array().unwrap().is_empty());

    let (_, uptime) = app
        .request_json(json_request("GET", "/api/v1/reports/uptime", None))
        .await;
    assert!(uptime["overall_uptime_percent"].is_null());
    assert!(uptime["monitors"].as_array().unwrap().is_empty());
}

// Silences an unused-import lint if json! is only used inside helpers.
#[allow(unused)]
fn _unused(_: Value) {}
