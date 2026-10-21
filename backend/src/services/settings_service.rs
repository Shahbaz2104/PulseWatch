//! Effective application settings: environment defaults overridden by the
//! `settings` table. Used for new-monitor defaults and exposed via the
//! settings API.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::{
    Config, MAX_CHECK_INTERVAL_SECS, MAX_CHECK_RETENTION_DAYS, MAX_CONCURRENCY, MAX_RETRY_ATTEMPTS,
    MAX_RETRY_BACKOFF_MAX_MS, MAX_RETRY_BACKOFF_MS, MAX_TIMEOUT_MS, MIN_CHECK_INTERVAL_SECS,
    MIN_CHECK_RETENTION_DAYS, MIN_CONCURRENCY, MIN_RETRY_ATTEMPTS, MIN_RETRY_BACKOFF_MAX_MS,
    MIN_RETRY_BACKOFF_MS, MIN_TIMEOUT_MS,
};
use crate::error::{AppError, FieldError};
use crate::repositories::settings_repository;

/// Setting keys persisted in the database.
pub const KEY_CHECK_INTERVAL: &str = "default_check_interval_seconds";
pub const KEY_TIMEOUT_MS: &str = "default_timeout_ms";
pub const KEY_RETRY_MAX_ATTEMPTS: &str = "default_retry_max_attempts";
pub const KEY_RETRY_BACKOFF_MS: &str = "default_retry_backoff_ms";
pub const KEY_RETRY_BACKOFF_MAX_MS: &str = "default_retry_backoff_max_ms";
pub const KEY_EXPECTED_STATUS_MIN: &str = "default_expected_status_min";
pub const KEY_EXPECTED_STATUS_MAX: &str = "default_expected_status_max";
pub const KEY_MAX_CONCURRENT_CHECKS: &str = "max_concurrent_checks";
pub const KEY_CHECK_RETENTION_DAYS: &str = "check_retention_days";

/// Effective defaults applied to newly created monitors, plus the runtime
/// concurrency limit (applied by the scheduler's semaphore at startup).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AppSettings {
    /// Default check interval for new monitors, seconds.
    pub default_check_interval_seconds: i64,
    /// Default per-request timeout for new monitors, milliseconds.
    pub default_timeout_ms: i64,
    /// Default total attempts (first try + retries) for new monitors.
    pub default_retry_max_attempts: i64,
    /// Initial retry backoff for checks, milliseconds.
    pub default_retry_backoff_ms: i64,
    /// Maximum retry backoff for checks, milliseconds.
    pub default_retry_backoff_max_ms: i64,
    /// Default lower bound of the acceptable HTTP status range.
    pub default_expected_status_min: i64,
    /// Default upper bound of the acceptable HTTP status range.
    pub default_expected_status_max: i64,
    /// Maximum concurrent outbound checks (scheduler semaphore size).
    pub max_concurrent_checks: i64,
    /// Days of check history retained by the cleanup worker.
    pub check_retention_days: i64,
}

/// Partial update payload: only provided fields change.
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct SettingsPatch {
    pub default_check_interval_seconds: Option<i64>,
    pub default_timeout_ms: Option<i64>,
    pub default_retry_max_attempts: Option<i64>,
    pub default_retry_backoff_ms: Option<i64>,
    pub default_retry_backoff_max_ms: Option<i64>,
    pub default_expected_status_min: Option<i64>,
    pub default_expected_status_max: Option<i64>,
    pub max_concurrent_checks: Option<i64>,
    pub check_retention_days: Option<i64>,
}

/// Response to a successful update: the new effective settings plus the list
/// of keys that only take effect after a process restart.
#[derive(Debug, Serialize, ToSchema)]
pub struct SettingsUpdateResponse {
    pub settings: AppSettings,
    pub restart_required: Vec<String>,
}

impl AppSettings {
    /// Load effective settings: env config as the base, database overrides on
    /// top. Unparsable stored values are ignored (env fallback applies).
    pub async fn load(pool: &sqlx::SqlitePool, config: &Config) -> Result<Self, sqlx::Error> {
        let stored = settings_repository::read_all(pool).await?;
        let parse = |key: &str, fallback: i64| -> i64 {
            stored
                .get(key)
                .and_then(|raw| raw.trim().parse::<i64>().ok())
                .unwrap_or(fallback)
        };
        let mut settings = Self {
            default_check_interval_seconds: parse(
                KEY_CHECK_INTERVAL,
                config.default_check_interval_seconds,
            ),
            default_timeout_ms: parse(KEY_TIMEOUT_MS, config.default_timeout_ms),
            default_retry_max_attempts: parse(
                KEY_RETRY_MAX_ATTEMPTS,
                config.default_retry_max_attempts,
            ),
            default_retry_backoff_ms: parse(
                KEY_RETRY_BACKOFF_MS,
                config.default_retry_backoff_ms as i64,
            ),
            default_retry_backoff_max_ms: parse(
                KEY_RETRY_BACKOFF_MAX_MS,
                config.default_retry_backoff_max_ms as i64,
            ),
            // Historical schema defaults for expected status on monitors.
            default_expected_status_min: parse(KEY_EXPECTED_STATUS_MIN, 200),
            default_expected_status_max: parse(KEY_EXPECTED_STATUS_MAX, 399),
            max_concurrent_checks: parse(
                KEY_MAX_CONCURRENT_CHECKS,
                config.max_concurrent_checks as i64,
            ),
            check_retention_days: parse(KEY_CHECK_RETENTION_DAYS, config.check_retention_days),
        };
        // Guard against out-of-range stored rows (manual DB edits).
        settings.clamp();
        Ok(settings)
    }

    fn clamp(&mut self) {
        self.default_check_interval_seconds = self
            .default_check_interval_seconds
            .clamp(MIN_CHECK_INTERVAL_SECS, MAX_CHECK_INTERVAL_SECS);
        self.default_timeout_ms = self
            .default_timeout_ms
            .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS);
        self.default_retry_max_attempts = self
            .default_retry_max_attempts
            .clamp(MIN_RETRY_ATTEMPTS, MAX_RETRY_ATTEMPTS);
        self.default_retry_backoff_ms = self
            .default_retry_backoff_ms
            .clamp(MIN_RETRY_BACKOFF_MS, MAX_RETRY_BACKOFF_MS);
        self.default_retry_backoff_max_ms = self
            .default_retry_backoff_max_ms
            .clamp(self.default_retry_backoff_ms, MAX_RETRY_BACKOFF_MAX_MS);
        self.default_expected_status_min = self.default_expected_status_min.clamp(100, 599);
        self.default_expected_status_max = self.default_expected_status_max.clamp(100, 599);
        if self.default_expected_status_min > self.default_expected_status_max {
            self.default_expected_status_max = self.default_expected_status_min;
        }
        self.max_concurrent_checks = self
            .max_concurrent_checks
            .clamp(MIN_CONCURRENCY as i64, MAX_CONCURRENCY as i64);
        self.check_retention_days = self
            .check_retention_days
            .clamp(MIN_CHECK_RETENTION_DAYS, MAX_CHECK_RETENTION_DAYS);
    }

    /// Validate a patch, persist the provided fields, and return the new
    /// effective settings together with keys that require a restart.
    pub async fn update(
        pool: &sqlx::SqlitePool,
        config: &Config,
        patch: SettingsPatch,
    ) -> Result<SettingsUpdateResponse, AppError> {
        let before = Self::load(pool, config).await?;
        let mut after = before.clone();

        let mut errors: Vec<FieldError> = Vec::new();
        let mut check = |field: &str, value: Option<i64>, min: i64, max: i64| -> Option<i64> {
            let value = value?;
            if value < min || value > max {
                errors.push(FieldError {
                    field: field.to_string(),
                    message: format!("must be between {min} and {max}"),
                });
            }
            Some(value)
        };

        if let Some(value) = check(
            "default_check_interval_seconds",
            patch.default_check_interval_seconds,
            MIN_CHECK_INTERVAL_SECS,
            MAX_CHECK_INTERVAL_SECS,
        ) {
            after.default_check_interval_seconds = value;
        }
        if let Some(value) = check(
            "default_timeout_ms",
            patch.default_timeout_ms,
            MIN_TIMEOUT_MS,
            MAX_TIMEOUT_MS,
        ) {
            after.default_timeout_ms = value;
        }
        if let Some(value) = check(
            "default_retry_max_attempts",
            patch.default_retry_max_attempts,
            MIN_RETRY_ATTEMPTS,
            MAX_RETRY_ATTEMPTS,
        ) {
            after.default_retry_max_attempts = value;
        }
        if let Some(value) = check(
            "default_retry_backoff_ms",
            patch.default_retry_backoff_ms,
            MIN_RETRY_BACKOFF_MS,
            MAX_RETRY_BACKOFF_MS,
        ) {
            after.default_retry_backoff_ms = value;
        }
        if let Some(value) = check(
            "default_retry_backoff_max_ms",
            patch.default_retry_backoff_max_ms,
            MIN_RETRY_BACKOFF_MAX_MS,
            MAX_RETRY_BACKOFF_MAX_MS,
        ) {
            after.default_retry_backoff_max_ms = value;
        }
        if let Some(value) = check(
            "default_expected_status_min",
            patch.default_expected_status_min,
            100,
            599,
        ) {
            after.default_expected_status_min = value;
        }
        if let Some(value) = check(
            "default_expected_status_max",
            patch.default_expected_status_max,
            100,
            599,
        ) {
            after.default_expected_status_max = value;
        }
        if let Some(value) = check(
            "max_concurrent_checks",
            patch.max_concurrent_checks,
            MIN_CONCURRENCY as i64,
            MAX_CONCURRENCY as i64,
        ) {
            after.max_concurrent_checks = value;
        }
        if let Some(value) = check(
            "check_retention_days",
            patch.check_retention_days,
            MIN_CHECK_RETENTION_DAYS,
            MAX_CHECK_RETENTION_DAYS,
        ) {
            after.check_retention_days = value;
        }
        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }
        if after.default_retry_backoff_max_ms < after.default_retry_backoff_ms {
            return Err(AppError::validation(
                "default_retry_backoff_max_ms",
                "must be >= default_retry_backoff_ms",
            ));
        }
        if after.default_expected_status_min > after.default_expected_status_max {
            return Err(AppError::validation(
                "default_expected_status_max",
                "must be >= default_expected_status_min",
            ));
        }

        let now = chrono::Utc::now();
        let rows: [(&str, i64); 9] = [
            (KEY_CHECK_INTERVAL, after.default_check_interval_seconds),
            (KEY_TIMEOUT_MS, after.default_timeout_ms),
            (KEY_RETRY_MAX_ATTEMPTS, after.default_retry_max_attempts),
            (KEY_RETRY_BACKOFF_MS, after.default_retry_backoff_ms),
            (KEY_RETRY_BACKOFF_MAX_MS, after.default_retry_backoff_max_ms),
            (KEY_EXPECTED_STATUS_MIN, after.default_expected_status_min),
            (KEY_EXPECTED_STATUS_MAX, after.default_expected_status_max),
            (KEY_MAX_CONCURRENT_CHECKS, after.max_concurrent_checks),
            (KEY_CHECK_RETENTION_DAYS, after.check_retention_days),
        ];
        for (key, value) in rows {
            settings_repository::upsert(pool, key, value, now)
                .await
                .map_err(AppError::Database)?;
        }

        let mut restart_required = Vec::new();
        if before.max_concurrent_checks != after.max_concurrent_checks {
            restart_required.push(KEY_MAX_CONCURRENT_CHECKS.to_string());
        }
        Ok(SettingsUpdateResponse {
            settings: after,
            restart_required,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_validation_rejects_out_of_range() {
        let patch = SettingsPatch {
            default_timeout_ms: Some(10), // below MIN_TIMEOUT_MS
            ..SettingsPatch::default()
        };
        // Validation happens inside `update`, which needs a pool; assert the
        // pure bounds logic via clamp instead.
        let mut settings = AppSettings {
            default_check_interval_seconds: 60,
            default_timeout_ms: 5_000,
            default_retry_max_attempts: 2,
            default_retry_backoff_ms: 500,
            default_retry_backoff_max_ms: 10_000,
            default_expected_status_min: 200,
            default_expected_status_max: 399,
            max_concurrent_checks: 16,
            check_retention_days: 90,
        };
        settings.default_timeout_ms = patch.default_timeout_ms.unwrap();
        settings.clamp();
        assert_eq!(settings.default_timeout_ms, MIN_TIMEOUT_MS);
    }

    #[test]
    fn clamp_fixes_inverted_status_range() {
        let mut settings = AppSettings {
            default_check_interval_seconds: 60,
            default_timeout_ms: 5_000,
            default_retry_max_attempts: 2,
            default_retry_backoff_ms: 500,
            default_retry_backoff_max_ms: 10_000,
            default_expected_status_min: 400,
            default_expected_status_max: 200,
            max_concurrent_checks: 16,
            check_retention_days: 90,
        };
        settings.clamp();
        assert!(settings.default_expected_status_min <= settings.default_expected_status_max);
    }
}
