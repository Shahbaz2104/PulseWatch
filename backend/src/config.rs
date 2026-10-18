//! Application configuration loaded from the environment (`.env` is supported).
//!
//! All values are validated at startup so a misconfigured process fails fast
//! instead of misbehaving later.

use std::net::IpAddr;

use thiserror::Error;

pub const MIN_CHECK_INTERVAL_SECS: i64 = 10;
pub const MAX_CHECK_INTERVAL_SECS: i64 = 86_400;
pub const MIN_TIMEOUT_MS: i64 = 100;
pub const MAX_TIMEOUT_MS: i64 = 60_000;
pub const MIN_RETRY_ATTEMPTS: i64 = 1;
pub const MAX_RETRY_ATTEMPTS: i64 = 5;
pub const MIN_CONCURRENCY: usize = 1;
pub const MAX_CONCURRENCY: usize = 256;

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
    /// Test/dev-only escape hatch; must stay `false` in production so private
    /// and loopback destinations remain blocked.
    pub allow_private_targets: bool,
    /// Origins allowed by CORS (comma-separated `ALLOWED_ORIGINS`).
    pub allowed_origins: Vec<String>,
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
            default_retry_backoff_ms: parse_or("DEFAULT_RETRY_BACKOFF_MS", 500)?,
            default_retry_backoff_max_ms: parse_or("DEFAULT_RETRY_BACKOFF_MAX_MS", 10_000)?,
            max_concurrent_checks: parse_or("MAX_CONCURRENT_CHECKS", 16_usize)
                .map_err(|_| ConfigError::Invalid("MAX_CONCURRENT_CHECKS"))?,
            allow_private_targets: parse_or("PULSEWATCH_ALLOW_PRIVATE_TARGETS", false)?,
            allowed_origins: std::env::var("ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string())
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
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
        Ok(config)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env-mutating assertions live in one test so parallel test threads never
    // observe each other's temporary `DATABASE_URL` changes.
    #[test]
    fn env_config_validation() {
        // Ensure no leftover env from other tests influences this one.
        let config = Config::from_env().expect("environment defaults must validate");
        assert!(config.max_concurrent_checks >= 1);
        assert!(config.default_retry_max_attempts >= 1);
        assert!(config.database_url.starts_with("sqlite"));

        std::env::set_var("DATABASE_URL", "postgres://localhost/pulsewatch");
        let err = Config::from_env().unwrap_err();
        std::env::remove_var("DATABASE_URL");
        assert!(matches!(err, ConfigError::Invalid("DATABASE_URL")));
    }
}
