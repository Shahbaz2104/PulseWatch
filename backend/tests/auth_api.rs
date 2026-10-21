//! Authentication contract: public reads, protected writes, signed cookies,
//! logout and read-only demo mode.

mod common;

use axum::http::{header::SET_COOKIE, HeaderMap, StatusCode};
use common::{create_body, json_request, TestApp, TEST_ADMIN_PASSWORD, TEST_SESSION_SECRET};

fn cookie_header(headers: &HeaderMap) -> String {
    headers
        .get(SET_COOKIE)
        .expect("Set-Cookie header")
        .to_str()
        .expect("ASCII cookie")
        .to_string()
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .trim()
        .to_string()
}

#[tokio::test]
async fn public_reads_stay_open_and_writes_require_login() {
    let app = TestApp::new(false).await;

    for uri in [
        "/api/v1/health",
        "/api/v1/monitors",
        "/api/v1/incidents",
        "/api/v1/reports/overview",
        "/api/v1/settings",
        "/api/v1/auth/status",
    ] {
        let (status, _) = app.request_json_anon(json_request("GET", uri, None)).await;
        assert_eq!(status, StatusCode::OK, "GET {uri} must be public");
    }

    let (status, body) = app
        .request_json_anon(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body("https://example.com", serde_json::json!({}))),
        ))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");

    let (status, body) = app
        .request_json_anon(json_request(
            "PUT",
            "/api/v1/settings",
            Some(serde_json::json!({ "default_check_interval_seconds": 120 })),
        ))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");

    let id = uuid::Uuid::new_v4();
    for (method, uri) in [
        ("PUT", format!("/api/v1/monitors/{id}")),
        ("DELETE", format!("/api/v1/monitors/{id}")),
        ("POST", format!("/api/v1/monitors/{id}/check")),
        ("POST", format!("/api/v1/monitors/{id}/pause")),
        ("POST", format!("/api/v1/monitors/{id}/resume")),
    ] {
        let (status, _) = app
            .request_json_anon(json_request(method, &uri, None))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }

    // The same create succeeds with the signed-in harness cookie.
    let (status, body) = app
        .request_json(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body("https://example.com", serde_json::json!({}))),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["url"], "https://example.com/");
}

#[tokio::test]
async fn login_issues_a_signed_http_only_cookie() {
    let app = TestApp::new(false).await;

    let (status, headers, body) = app
        .request_json_anon_with_headers(json_request("GET", "/api/v1/auth/status", None))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], false);
    assert_eq!(body["demo_mode"], false);
    assert_eq!(headers["cache-control"], "no-store");

    let (status, headers, body) = app
        .request_json_anon_with_headers(json_request(
            "POST",
            "/api/v1/auth/login",
            Some(serde_json::json!({ "password": "wrong-password" })),
        ))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
    assert!(headers.get(SET_COOKIE).is_none());

    let (status, headers, body) = app
        .request_json_anon_with_headers(json_request(
            "POST",
            "/api/v1/auth/login",
            Some(serde_json::json!({ "password": TEST_ADMIN_PASSWORD })),
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
    let set_cookie = cookie_header(&headers);
    assert!(set_cookie.contains("pulsewatch_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Path=/"));
    assert!(set_cookie.contains("Max-Age=604800"));
    assert!(
        !set_cookie.contains("Secure"),
        "dev default must work over HTTP"
    );
    assert_eq!(headers["cache-control"], "no-store");

    // A fresh cookie issued by login authenticates subsequent requests.
    let login_cookie = cookie_pair(&set_cookie);
    let mut status_request = json_request("GET", "/api/v1/auth/status", None);
    status_request
        .headers_mut()
        .insert("cookie", login_cookie.parse().unwrap());
    let (status, body) = app.request_json_anon(status_request).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
}

#[tokio::test]
async fn tampered_or_missing_cookie_is_rejected() {
    let app = TestApp::new(false).await;
    let token = app.session_cookie.clone().expect("session");
    let tampered = format!("{token}x");

    let mut request = json_request(
        "POST",
        "/api/v1/monitors",
        Some(create_body("https://example.com", serde_json::json!({}))),
    );
    request.headers_mut().insert(
        "cookie",
        format!("pulsewatch_session={tampered}").parse().unwrap(),
    );
    let (status, body) = app.request_json_anon(request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");

    let (status, body) = app
        .request_json_anon(json_request(
            "POST",
            "/api/v1/auth/login",
            Some(serde_json::json!({ "password": TEST_ADMIN_PASSWORD })),
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], true);
}

#[tokio::test]
async fn logout_clears_the_cookie() {
    let app = TestApp::new(false).await;
    let (status, headers, _) = app
        .request_json_with_headers(json_request("POST", "/api/v1/auth/logout", None))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let set_cookie = cookie_header(&headers);
    assert!(set_cookie.starts_with("pulsewatch_session=;"));
    assert!(set_cookie.contains("Max-Age=0"));
    assert!(set_cookie.contains("HttpOnly"));
    assert_eq!(headers["cache-control"], "no-store");
}

#[tokio::test]
async fn demo_mode_is_read_only_and_disables_login() {
    let app = TestApp::demo(false).await;
    assert!(app.session_cookie.is_none());
    assert!(app.state.config.auth.is_none());

    let (status, body) = app
        .request_json_anon(json_request("GET", "/api/v1/health", None))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["demo_mode"], true);

    let (status, body) = app
        .request_json_anon(json_request("GET", "/api/v1/auth/status", None))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authenticated"], false);
    assert_eq!(body["demo_mode"], true);

    let (status, _) = app
        .request_json_anon(json_request("GET", "/api/v1/monitors", None))
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .request_json_anon(json_request(
            "POST",
            "/api/v1/auth/login",
            Some(serde_json::json!({ "password": TEST_ADMIN_PASSWORD })),
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    let (status, body) = app
        .request_json_anon(json_request(
            "POST",
            "/api/v1/monitors",
            Some(create_body("https://example.com", serde_json::json!({}))),
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    let (status, _) = app
        .request_json_anon(json_request(
            "PUT",
            "/api/v1/settings",
            Some(serde_json::json!({ "default_check_interval_seconds": 120 })),
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app
        .request_json_anon(json_request(
            "POST",
            &format!("/api/v1/monitors/{}/check", uuid::Uuid::new_v4()),
            None,
        ))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn session_secret_produces_stable_signed_tokens() {
    let app = TestApp::new(false).await;
    let token = app.session_cookie.as_deref().expect("session");
    assert!(
        !token.contains(TEST_SESSION_SECRET),
        "token must not embed the secret"
    );
    assert_eq!(token.split('.').count(), 2);
}
