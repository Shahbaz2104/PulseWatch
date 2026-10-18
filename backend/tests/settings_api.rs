//! Integration tests for the settings API.

mod common;

use common::*;
use serde_json::json;

#[tokio::test]
async fn get_returns_env_derived_defaults() {
    let app = TestApp::new(true).await;
    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/settings", None))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["default_check_interval_seconds"], 60);
    assert_eq!(body["default_timeout_ms"], 5000);
    assert_eq!(body["default_retry_max_attempts"], 2);
    assert_eq!(body["default_expected_status_min"], 200);
    assert_eq!(body["default_expected_status_max"], 399);
    // TestApp builds its config with a semaphore of 8.
    assert_eq!(body["max_concurrent_checks"], 8);
}

#[tokio::test]
async fn update_round_trips_and_new_monitors_use_overrides() {
    let app = TestApp::new(true).await;

    let (status, body) = app
        .request_json(json_request(
            "PUT",
            "/api/v1/settings",
            Some(json!({
                "default_check_interval_seconds": 120,
                "default_timeout_ms": 2500,
            })),
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["settings"]["default_check_interval_seconds"], 120);
    assert_eq!(body["settings"]["default_timeout_ms"], 2500);
    // Untouched keys keep their previous values.
    assert_eq!(body["settings"]["default_retry_max_attempts"], 2);
    assert!(body["restart_required"].as_array().unwrap().is_empty());

    // GET reflects the override.
    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/settings", None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["default_check_interval_seconds"], 120);

    // A monitor created without explicit config picks up the override.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(json!({
                "name": "settings-defaults",
                "url": "http://example.com/uses-settings",
            })),
        ))
        .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["check_interval_seconds"], 120);
    assert_eq!(body["timeout_ms"], 2500);
    assert_eq!(body["retry_max_attempts"], 2);
    assert_eq!(body["expected_status_min"], 200);
    assert_eq!(body["expected_status_max"], 399);
}

#[tokio::test]
async fn concurrency_change_reports_restart_required() {
    let app = TestApp::new(true).await;
    let (status, body) = app
        .request_json(json_request(
            "PUT",
            "/api/v1/settings",
            Some(json!({ "max_concurrent_checks": 16 })),
        ))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["settings"]["max_concurrent_checks"], 16);
    let required = body["restart_required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "max_concurrent_checks");
}

#[tokio::test]
async fn invalid_settings_are_rejected_with_field_errors() {
    let app = TestApp::new(true).await;

    // Timeout below the documented minimum.
    let (status, body) = app
        .request_json(json_request(
            "PUT",
            "/api/v1/settings",
            Some(json!({ "default_timeout_ms": 10 })),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(body["error"]["code"], "validation_failed");
    assert_eq!(body["error"]["details"][0]["field"], "default_timeout_ms");

    // Inverted expected-status range.
    let (status, body) = app
        .request_json(json_request(
            "PUT",
            "/api/v1/settings",
            Some(json!({
                "default_expected_status_min": 400,
                "default_expected_status_max": 200,
            })),
        ))
        .await;
    assert_eq!(status, 422);
    assert_eq!(
        body["error"]["details"][0]["field"],
        "default_expected_status_max"
    );

    // Nothing was persisted by the failed updates.
    let (status, body) = app
        .request_json(json_request("GET", "/api/v1/settings", None))
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["default_timeout_ms"], 5000);
    assert_eq!(body["default_expected_status_min"], 200);
}
