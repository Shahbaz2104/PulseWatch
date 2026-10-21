//! Application configuration loaded from the environment (`.env` is supported).
//!
//! All values are validated at startup so a misconfigured process fails fast
//! instead of misbehaving later.

use std::net::IpAddr;
use std::path::PathBuf;

use argon2::password_hash::PasswordHash;
use thiserror::Error;

pub const MIN_CHECK_INTERVAL_SECS: i64 = 10;
pub const MAX_CHECK_INTERVAL_SECS: i64 = 86_400;
pub const MIN_TIMEOUT_MS: i64 = 100;
pub const MAX_TIMEOUT_MS: i64 = 60_000;
pub const MIN_RETRY_ATTEMPTS: i64 = 1;
pub const MAX_RETRY_ATTEMPTS: i64 = 5;
pub const MIN_RETRY_BACKOFF_MS: i64 = 0;
pub const MAX_RETRY_BACKOFF_MS: i64 = 60_000;
pub const MIN_RETRY_BACKOFF_MAX_MS: i64 = 0;
pub const MAX_RETRY_BACKOFF_MAX_MS: i64 = 300_000;
pub const MIN_CONCURRENCY: usize = 1;
pub const MAX_CONCURRENCY: usize = 256;
pub const MIN_CHECK_RETENTION_DAYS: i64 = 7;
pub const MAX_CHECK_RETENTION_DAYS: i64 = 365;
pub const DEFAULT_CHECK_RETENTION_DAYS: i64 = 90;

/// TLS mode for SMTP delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    StartTls,
    Implicit,
    None,
}

/// Single-admin credentials. Sessions are HMAC-signed cookies built from the
/// secret; the password itself is only ever stored as an Argon2id hash.
#[derive(Clone)]
pub struct AuthConfig {
    pub password_hash: String,
    pub session_secret: String,
    pub cookie_secure: bool,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("password_hash", &"<redacted>")
            .field("session_secret", &"<redacted>")
            .field("cookie_secure", &self.cookie_secure)
            .finish()
    }
}

/// Optional SMTP transport configuration. Secrets stay in the environment.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
    pub tls: SmtpTls,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing or invalid environment variable {0}")]
    Invalid(&'static str),
    #[error("{0} must be between {1} and {2}")]
    OutOfRange(&'static str, i64, i64),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub database_url: String,
    pub default_check_interval_seconds: i64,
    pub default_timeout_ms: i64,
    pub default_retry_max_attempts: i64,
    pub default_retry_backoff_ms: u64,
    pub default_retry_backoff_max_ms: u64,
    pub max_concurrent_checks: usize,
    /// Days of check history kept by the retention worker.
    pub check_retention_days: i64,
    /// Test/dev-only escape hatch; must stay `false` in production so private
    /// and loopback destinations remain blocked.
    pub allow_private_targets: bool,
    /// Origins allowed by CORS (comma-separated `ALLOWED_ORIGINS`).
    pub allowed_origins: Vec<String>,
    /// Optional SMTP transport for email alerts.
    pub smtp: Option<SmtpConfig>,
    /// Read-only public demo. Disables auth and rejects every mutation.
    pub demo_mode: bool,
    /// Optional built frontend directory served with SPA fallback.
    pub static_dir: Option<PathBuf>,
    /// `None` only in demo mode; otherwise startup requires valid credentials.
    pub auth: Option<AuthConfig>,
}

fn parse_or<T: std::str::FromStr>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => {
            v.trim().parse::<T>().map_err(|_| ConfigError::Invalid(key))
        }
        _ => Ok(default),
    }
}

fn in_range(key: &'static str, value: i64, min: i64, max: i64) -> Result<i64, ConfigError> {
    if value < min || value > max {
        return Err(ConfigError::OutOfRange(key, min, max));
    }
    Ok(value)
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let demo_mode = parse_or("DEMO_MODE", false)?;
        let config = Self {
            host: parse_or("HOST", "127.0.0.1".parse().unwrap())?,
            port: parse_or("PORT", 3000)?,
            database_url: parse_or("DATABASE_URL", "sqlite://./data/pulsewatch.db".to_string())?,
            default_check_interval_seconds: in_range(
                "DEFAULT_CHECK_INTERVAL_SECONDS",
                parse_or("DEFAULT_CHECK_INTERVAL_SECONDS", 60)?,
                MIN_CHECK_INTERVAL_SECS,
                MAX_CHECK_INTERVAL_SECS,
            )?,
            default_timeout_ms: in_range(
                "DEFAULT_TIMEOUT_MS",
                parse_or("DEFAULT_TIMEOUT_MS", 5_000)?,
                MIN_TIMEOUT_MS,
                MAX_TIMEOUT_MS,
            )?,
            default_retry_max_attempts: in_range(
                "DEFAULT_RETRY_MAX_ATTEMPTS",
                parse_or("DEFAULT_RETRY_MAX_ATTEMPTS", 2)?,
                MIN_RETRY_ATTEMPTS,
                MAX_RETRY_ATTEMPTS,
            )?,
            default_retry_backoff_ms: in_range(
                "DEFAULT_RETRY_BACKOFF_MS",
                parse_or("DEFAULT_RETRY_BACKOFF_MS", 500)?,
                MIN_RETRY_BACKOFF_MS,
                MAX_RETRY_BACKOFF_MS,
            )? as u64,
            default_retry_backoff_max_ms: in_range(
                "DEFAULT_RETRY_BACKOFF_MAX_MS",
                parse_or("DEFAULT_RETRY_BACKOFF_MAX_MS", 10_000)?,
                MIN_RETRY_BACKOFF_MAX_MS,
                MAX_RETRY_BACKOFF_MAX_MS,
            )? as u64,
            max_concurrent_checks: parse_or("MAX_CONCURRENT_CHECKS", 16_usize)
                .map_err(|_| ConfigError::Invalid("MAX_CONCURRENT_CHECKS"))?,
            check_retention_days: in_range(
                "CHECK_RETENTION_DAYS",
                parse_or("CHECK_RETENTION_DAYS", DEFAULT_CHECK_RETENTION_DAYS)?,
                MIN_CHECK_RETENTION_DAYS,
                MAX_CHECK_RETENTION_DAYS,
            )?,
            allow_private_targets: parse_or("PULSEWATCH_ALLOW_PRIVATE_TARGETS", false)?,
            allowed_origins: std::env::var("ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string())
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            smtp: load_smtp()?,
            demo_mode,
            static_dir: load_static_dir(),
            auth: load_auth(demo_mode)?,
        };

        if config.max_concurrent_checks < MIN_CONCURRENCY
            || config.max_concurrent_checks > MAX_CONCURRENCY
        {
            return Err(ConfigError::OutOfRange(
                "MAX_CONCURRENT_CHECKS",
                MIN_CONCURRENCY as i64,
                MAX_CONCURRENCY as i64,
            ));
        }
        if !config.database_url.starts_with("sqlite") {
            return Err(ConfigError::Invalid("DATABASE_URL"));
        }
        if config.default_retry_backoff_max_ms < config.default_retry_backoff_ms {
            return Err(ConfigError::Invalid("DEFAULT_RETRY_BACKOFF_MAX_MS"));
        }
        if let Some(dir) = &config.static_dir {
            if !dir.is_dir() {
                return Err(ConfigError::Invalid("STATIC_DIR"));
            }
        }
        Ok(config)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn email_configured(&self) -> bool {
        self.smtp.is_some()
    }
}

fn load_static_dir() -> Option<PathBuf> {
    std::env::var("STATIC_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn load_auth(demo_mode: bool) -> Result<Option<AuthConfig>, ConfigError> {
    if demo_mode {
        return Ok(None);
    }

    let password_hash = std::env::var("ADMIN_PASSWORD_HASH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(ConfigError::Invalid("ADMIN_PASSWORD_HASH"))?;
    if !password_hash.starts_with("$argon2id$") || PasswordHash::new(&password_hash).is_err() {
        return Err(ConfigError::Invalid("ADMIN_PASSWORD_HASH"));
    }

    let session_secret = std::env::var("SESSION_SECRET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| value.len() >= 32)
        .ok_or(ConfigError::Invalid("SESSION_SECRET"))?;

    Ok(Some(AuthConfig {
        password_hash,
        session_secret,
        cookie_secure: parse_or("SESSION_COOKIE_SECURE", false)?,
    }))
}

fn load_smtp() -> Result<Option<SmtpConfig>, ConfigError> {
    let host = std::env::var("SMTP_HOST")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let has_any_other = [
        "SMTP_PORT",
        "SMTP_USERNAME",
        "SMTP_PASSWORD",
        "SMTP_FROM",
        "SMTP_TLS",
    ]
    .iter()
    .any(|key| std::env::var(key).is_ok());
    let Some(host) = host else {
        if has_any_other {
            return Err(ConfigError::Invalid("SMTP_HOST"));
        }
        return Ok(None);
    };

    let port = parse_or("SMTP_PORT", 587_u16)?;
    if port == 0 {
        return Err(ConfigError::Invalid("SMTP_PORT"));
    }
    let from = std::env::var("SMTP_FROM")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(ConfigError::Invalid("SMTP_FROM"))?;
    if from.len() > 254 || from.chars().any(char::is_whitespace) || !from.contains('@') {
        return Err(ConfigError::Invalid("SMTP_FROM"));
    }
    let username = std::env::var("SMTP_USERNAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let password = std::env::var("SMTP_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    if username.is_some() != password.is_some() {
        return Err(ConfigError::Invalid("SMTP_PASSWORD"));
    }
    let tls = match std::env::var("SMTP_TLS")
        .unwrap_or_else(|_| "starttls".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "starttls" | "start-tls" => SmtpTls::StartTls,
        "implicit" | "tls" => SmtpTls::Implicit,
        "none" | "plain" => SmtpTls::None,
        _ => return Err(ConfigError::Invalid("SMTP_TLS")),
    };
    Ok(Some(SmtpConfig {
        host,
        port,
        username,
        password,
        from,
        tls,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$EeXitryqiZtfPq+UAMZH1w$gNjv7ZQsQQjXqNSWo04WrFoLy2PmcjhXk1IXqNc0dwE";
    const TEST_SECRET: &str = "test-session-secret-at-least-32-bytes";

    fn set_auth_env() {
        std::env::remove_var("DEMO_MODE");
        std::env::set_var("ADMIN_PASSWORD_HASH", TEST_HASH);
        std::env::set_var("SESSION_SECRET", TEST_SECRET);
        std::env::set_var("SESSION_COOKIE_SECURE", "false");
    }

    fn clear_auth_env() {
        for key in [
            "DEMO_MODE",
            "ADMIN_PASSWORD_HASH",
            "SESSION_SECRET",
            "SESSION_COOKIE_SECURE",
        ] {
            std::env::remove_var(key);
        }
    }

    // Env-mutating assertions live in one test so parallel test threads never
    // observe each other's temporary `DATABASE_URL` changes.
    #[test]
    fn env_config_validation() {
        set_auth_env();
        let config = Config::from_env().expect("environment defaults must validate");
        assert!(config.max_concurrent_checks >= 1);
        assert!(config.default_retry_max_attempts >= 1);
        assert!(config.database_url.starts_with("sqlite"));
        assert!(!config.demo_mode);
        assert!(config.auth.is_some());
        assert_eq!(config.check_retention_days, 90);

        std::env::set_var("CHECK_RETENTION_DAYS", "6");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(
            err,
            ConfigError::OutOfRange("CHECK_RETENTION_DAYS", 7, 365)
        ));
        std::env::set_var("CHECK_RETENTION_DAYS", "365");
        assert_eq!(Config::from_env().unwrap().check_retention_days, 365);
        std::env::remove_var("CHECK_RETENTION_DAYS");

        std::env::set_var("DATABASE_URL", "postgres://localhost/pulsewatch");
        let err = Config::from_env().unwrap_err();
        std::env::remove_var("DATABASE_URL");
        assert!(matches!(err, ConfigError::Invalid("DATABASE_URL")));

        std::env::set_var("STATIC_DIR", "/definitely/not/a/real/directory");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid("STATIC_DIR")));
        let static_dir = tempfile::tempdir().expect("temp static dir");
        std::env::set_var("STATIC_DIR", static_dir.path());
        assert_eq!(
            Config::from_env().unwrap().static_dir.as_deref(),
            Some(static_dir.path())
        );
        std::env::remove_var("STATIC_DIR");

        assert!(config.smtp.is_none());

        for key in [
            "SMTP_HOST",
            "SMTP_PORT",
            "SMTP_USERNAME",
            "SMTP_PASSWORD",
            "SMTP_FROM",
            "SMTP_TLS",
        ] {
            std::env::remove_var(key);
        }
        std::env::set_var("SMTP_HOST", "smtp.example.com");
        std::env::set_var("SMTP_FROM", "alerts@example.com");
        std::env::set_var("SMTP_TLS", "implicit");
        let smtp = Config::from_env()
            .expect("complete SMTP config")
            .smtp
            .expect("SMTP configured");
        assert_eq!(smtp.port, 587);
        assert_eq!(smtp.tls, SmtpTls::Implicit);
        assert!(smtp.username.is_none());

        std::env::remove_var("SMTP_FROM");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid("SMTP_FROM")));
        for key in [
            "SMTP_HOST",
            "SMTP_PORT",
            "SMTP_USERNAME",
            "SMTP_PASSWORD",
            "SMTP_FROM",
            "SMTP_TLS",
        ] {
            std::env::remove_var(key);
        }

        // Authentication is mandatory outside demo mode.
        std::env::remove_var("ADMIN_PASSWORD_HASH");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid("ADMIN_PASSWORD_HASH")));

        std::env::set_var("ADMIN_PASSWORD_HASH", "plain-text-password");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid("ADMIN_PASSWORD_HASH")));

        std::env::set_var("ADMIN_PASSWORD_HASH", TEST_HASH);
        std::env::set_var("SESSION_SECRET", "too-short");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::Invalid("SESSION_SECRET")));

        // Demo mode skips credentials entirely.
        clear_auth_env();
        std::env::set_var("DEMO_MODE", "true");
        let config = Config::from_env().expect("demo mode needs no credentials");
        assert!(config.demo_mode);
        assert!(config.auth.is_none());
        clear_auth_env();
    }
}
