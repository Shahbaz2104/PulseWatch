//! Single-admin session authentication.
//!
//! Sessions are stateless HMAC-SHA256 tokens carried in an HttpOnly,
//! SameSite=Strict cookie. Reads stay public; every mutating handler declares
//! the [`AdminSession`] extractor, which rejects unauthenticated requests with
//! `401` and all writes with `403` in demo mode.

use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{PasswordHash, PasswordVerifier};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::header::COOKIE;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

/// Session cookie name.
pub const SESSION_COOKIE: &str = "pulsewatch_session";
/// Session lifetime: seven days.
pub const SESSION_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const SESSION_SUBJECT: &str = "admin";
/// Tolerated clock skew when checking the issued-at claim.
const CLOCK_SKEW_SECONDS: i64 = 60;

type HmacSha256 = Hmac<Sha256>;

/// Extractor that admits an authenticated admin. Always rejects in demo mode
/// because the demo is read-only.
pub struct AdminSession;

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if state.config.demo_mode {
            return Err(AppError::Forbidden("demo mode is read-only".to_string()));
        }
        if is_authenticated(&state.config, &parts.headers) {
            Ok(AdminSession)
        } else {
            Err(AppError::Unauthorized(
                "authentication required".to_string(),
            ))
        }
    }
}

/// Verify the submitted password against the configured Argon2id hash.
pub fn verify_password(config: &Config, password: &str) -> bool {
    let Some(auth) = config.auth.as_ref() else {
        return false;
    };
    let Ok(hash) = PasswordHash::new(&auth.password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .is_ok()
}

/// Issue a signed session token. `None` when auth is disabled (demo mode).
pub fn issue_session(config: &Config) -> Option<String> {
    let auth = config.auth.as_ref()?;
    let now = now_unix();
    let expires_at = now + SESSION_TTL_SECONDS;
    let payload = format!("{SESSION_SUBJECT}:{now}:{expires_at}");
    let signature = sign(auth.session_secret.as_bytes(), payload.as_bytes());
    Some(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload.as_bytes()),
        URL_SAFE_NO_PAD.encode(signature)
    ))
}

/// Whether the request carries a valid, unexpired session cookie.
pub fn is_authenticated(config: &Config, headers: &HeaderMap) -> bool {
    let Some(auth) = config.auth.as_ref() else {
        return false;
    };
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else {
        return false;
    };
    verify_session(token, &auth.session_secret)
}

/// `Set-Cookie` value for a successful login.
pub fn session_cookie(config: &Config, token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_TTL_SECONDS}{}",
        secure_attribute(config)
    )
}

/// `Set-Cookie` value that removes the session cookie.
pub fn clear_cookie(config: &Config) -> String {
    format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{}",
        secure_attribute(config)
    )
}

fn secure_attribute(config: &Config) -> &'static str {
    if config.auth.as_ref().is_some_and(|auth| auth.cookie_secure) {
        "; Secure"
    } else {
        ""
    }
}

fn verify_session(token: &str, secret: &str) -> bool {
    let Some((payload_b64, signature_b64)) = token.split_once('.') else {
        return false;
    };
    let Ok(payload) = URL_SAFE_NO_PAD.decode(payload_b64) else {
        return false;
    };
    let Ok(signature) = URL_SAFE_NO_PAD.decode(signature_b64) else {
        return false;
    };

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(&payload);
    if mac.verify_slice(&signature).is_err() {
        return false;
    }

    let Ok(payload) = std::str::from_utf8(&payload) else {
        return false;
    };
    let mut claims = payload.split(':');
    let subject = claims.next();
    let issued_at = claims.next().and_then(|value| value.parse::<i64>().ok());
    let expires_at = claims.next().and_then(|value| value.parse::<i64>().ok());
    if claims.next().is_some() {
        return false;
    }

    let now = now_unix();
    subject == Some(SESSION_SUBJECT)
        && issued_at.is_some_and(|issued| issued <= now + CLOCK_SKEW_SECONDS)
        && expires_at.is_some_and(|expires| expires > now)
}

fn sign(secret: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(payload);
    mac.finalize().into_bytes().to_vec()
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|header| header.split(';'))
        .find_map(|pair| {
            let (cookie_name, value) = pair.trim().split_once('=')?;
            (cookie_name == name && !value.is_empty()).then_some(value)
        })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Build a session cookie response header, exposing helpers for route handlers.
pub fn cookie_header(value: &str) -> HeaderValue {
    HeaderValue::from_str(value).expect("cookie values are header-safe")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, Config};

    fn config() -> Config {
        Config {
            host: "127.0.0.1".parse().unwrap(),
            port: 3000,
            database_url: "sqlite::memory:".to_string(),
            default_check_interval_seconds: 60,
            default_timeout_ms: 5_000,
            default_retry_max_attempts: 2,
            default_retry_backoff_ms: 500,
            default_retry_backoff_max_ms: 10_000,
            max_concurrent_checks: 16,
            check_retention_days: 90,
            allow_private_targets: false,
            allowed_origins: Vec::new(),
            smtp: None,
            demo_mode: false,
            static_dir: None,
            auth: Some(AuthConfig {
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$test".to_string(),
                session_secret: "a".repeat(32),
                cookie_secure: true,
            }),
        }
    }

    #[test]
    fn sessions_round_trip_and_reject_tampering() {
        let config = config();
        let token = issue_session(&config).expect("session");
        assert!(verify_session(
            &token,
            &config.auth.as_ref().unwrap().session_secret
        ));

        let tampered = format!("{token}x");
        assert!(!verify_session(
            &tampered,
            &config.auth.as_ref().unwrap().session_secret
        ));
        assert!(!verify_session(&token, "b".repeat(32).as_str()));
        assert!(!verify_session("not-a-token", "secret"));
    }

    #[test]
    fn cookies_are_http_only_strict_and_secure() {
        let config = config();
        let token = issue_session(&config).unwrap();
        let cookie = session_cookie(&config, &token);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("Max-Age=604800"));

        let cleared = clear_cookie(&config);
        assert!(cleared.contains("Max-Age=0"));
    }
}
