//! Bounded retry policy shared by scheduled and manual checks.
//!
//! A check makes at most [`RetryPolicy::max_attempts`] attempts. Backoff is
//! exponential from the monitor's configured initial delay, capped, and by
//! default fully jittered (uniform in `[0, backoff]`). Only *transient*
//! failures are retried; a stable status mismatch (for example 404) is
//! recorded immediately.

use std::time::Duration;

use crate::models::check_result::CheckErrorType;
use crate::models::monitor::Monitor;
use crate::services::settings_service::AppSettings;

/// Responses that indicate a temporary condition even though an HTTP response
/// was received. Everything else failing the status policy is treated as
/// stable and never retried.
pub const RETRYABLE_HTTP_STATUSES: [u16; 4] = [429, 502, 503, 504];

/// Statuses whose `Retry-After` header (when present and sane) overrides the
/// computed backoff.
const RETRY_AFTER_STATUSES: [u16; 2] = [429, 503];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts including the first; always >= 1.
    pub max_attempts: i64,
    /// Delay before the second attempt (before jitter).
    pub initial_backoff_ms: u64,
    /// Upper bound applied to every computed backoff.
    pub max_backoff_ms: u64,
    /// Full jitter when `true`: sleep a uniform random slice of the backoff.
    pub jitter: bool,
}

impl RetryPolicy {
    pub fn new(max_attempts: i64, initial_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            initial_backoff_ms,
            max_backoff_ms: max_backoff_ms.max(initial_backoff_ms),
            jitter: true,
        }
    }

    /// Retry settings for one monitor: attempts come from the monitor row,
    /// backoff timing from the effective application settings.
    pub fn for_monitor(monitor: &Monitor, settings: &AppSettings) -> Self {
        let attempts = if monitor.retry_max_attempts > 0 {
            monitor.retry_max_attempts
        } else {
            settings.default_retry_max_attempts
        };
        Self::new(
            attempts,
            settings.default_retry_backoff_ms.max(0) as u64,
            settings.default_retry_backoff_max_ms.max(0) as u64,
        )
    }

    /// Upper bound as a `Duration` (used to clamp `Retry-After`).
    pub fn max_backoff_millis(&self) -> Duration {
        Duration::from_millis(self.max_backoff_ms)
    }

    /// Deterministic (jitter-free) exponential backoff after attempt number
    /// `failed_attempt` (1-based) failed: `initial * 2^(failed_attempt - 1)`,
    /// capped at `max_backoff_ms`.
    pub fn backoff_base(&self, failed_attempt: i64) -> Duration {
        let exponent = (failed_attempt.max(1) - 1).min(31) as u32;
        let factor = 2u64.saturating_pow(exponent);
        let millis = self.initial_backoff_ms.saturating_mul(factor);
        Duration::from_millis(millis.min(self.max_backoff_ms))
    }

    /// Backoff actually slept: full jitter over `[0, backoff_base]`.
    pub fn backoff(&self, failed_attempt: i64) -> Duration {
        let base = self.backoff_base(failed_attempt);
        if !self.jitter || base.is_zero() {
            return base;
        }
        let max_ms = base.as_millis() as u64;
        Duration::from_millis(rand::random_range(0..=max_ms))
    }

    /// Whether another attempt is worth making after a failure with the given
    /// classification and (optional) received HTTP status, given that
    /// `attempts_so_far` attempts have already been made.
    ///
    /// Retryable: transport-transient classes (`is_transient()`), plus the
    /// status codes in [`RETRYABLE_HTTP_STATUSES`] regardless of the generic
    /// `HttpStatusMismatch` classification.
    pub fn should_retry(
        &self,
        error_type: CheckErrorType,
        http_status: Option<i64>,
        attempts_so_far: i64,
    ) -> bool {
        if attempts_so_far >= self.max_attempts {
            return false;
        }
        let status_transient = http_status
            .and_then(|status| u16::try_from(status).ok())
            .is_some_and(|status| RETRYABLE_HTTP_STATUSES.contains(&status));
        status_transient || error_type.is_transient()
    }

    /// Honor `Retry-After` (delta-seconds or HTTP-date) for the given status,
    /// clamped to `cap`. Returns `None` when the header is absent, malformed,
    /// or the status does not use it.
    pub fn retry_after_delay(
        header: Option<&str>,
        status: Option<i64>,
        cap: Duration,
    ) -> Option<Duration> {
        let status = u16::try_from(status?).ok()?;
        if !RETRY_AFTER_STATUSES.contains(&status) {
            return None;
        }
        let header = header?.trim();
        if header.is_empty() {
            return None;
        }
        let delay = if let Ok(seconds) = header.parse::<u64>() {
            Duration::from_secs(seconds)
        } else if let Ok(when) = chrono::DateTime::parse_from_rfc2822(header) {
            let delta = when.with_timezone(&chrono::Utc) - chrono::Utc::now();
            delta.to_std().unwrap_or(Duration::ZERO)
        } else {
            return None;
        };
        Some(delay.min(cap))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RetryPolicy {
        RetryPolicy::new(3, 500, 10_000)
    }

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        let policy = policy();
        assert_eq!(policy.backoff_base(1), Duration::from_millis(500));
        assert_eq!(policy.backoff_base(2), Duration::from_millis(1_000));
        assert_eq!(policy.backoff_base(3), Duration::from_millis(2_000));
        assert_eq!(policy.backoff_base(10), Duration::from_millis(10_000));
        assert_eq!(policy.backoff_base(100), Duration::from_millis(10_000));
        // Attempt numbers below 1 are clamped.
        assert_eq!(policy.backoff_base(0), Duration::from_millis(500));
        assert_eq!(policy.backoff_base(-5), Duration::from_millis(500));
    }

    #[test]
    fn backoff_never_exceeds_base_with_jitter() {
        let policy = policy();
        for attempt in 1..=5 {
            let base = policy.backoff_base(attempt);
            for _ in 0..50 {
                assert!(policy.backoff(attempt) <= base);
            }
        }
    }

    #[test]
    fn jitter_disabled_returns_exact_base() {
        let mut policy = policy();
        policy.jitter = false;
        assert_eq!(policy.backoff(2), Duration::from_millis(1_000));
    }

    #[test]
    fn constructor_clamps_attempts_and_orders_cap() {
        let policy = RetryPolicy::new(0, 5_000, 1_000);
        assert_eq!(policy.max_attempts, 1);
        assert_eq!(policy.max_backoff_ms, 5_000);
    }

    #[test]
    fn should_retry_classification() {
        let policy = RetryPolicy::new(2, 100, 1_000);

        // Transient transport failures retry until the attempt budget is gone.
        assert!(policy.should_retry(CheckErrorType::Timeout, None, 1));
        assert!(policy.should_retry(CheckErrorType::Connection, None, 1));
        assert!(policy.should_retry(CheckErrorType::Dns, None, 1));
        assert!(!policy.should_retry(CheckErrorType::Timeout, None, 2));

        // Stable failures never retry, even with budget left.
        assert!(!policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(404), 1));
        assert!(!policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(500), 1));
        assert!(!policy.should_retry(CheckErrorType::Tls, None, 1));
        assert!(!policy.should_retry(CheckErrorType::BlockedTarget, None, 1));
        assert!(!policy.should_retry(CheckErrorType::TooManyRedirects, None, 1));

        // Selected statuses are transient despite a response being received.
        assert!(policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(429), 1));
        assert!(policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(503), 1));
        assert!(policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(502), 1));
        assert!(policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(504), 1));
        assert!(!policy.should_retry(CheckErrorType::HttpStatusMismatch, Some(429), 2));
    }

    #[test]
    fn retry_after_parses_seconds_and_dates() {
        let cap = Duration::from_secs(30);

        assert_eq!(
            RetryPolicy::retry_after_delay(Some("7"), Some(503), cap),
            Some(Duration::from_secs(7))
        );
        // Clamped to the cap.
        assert_eq!(
            RetryPolicy::retry_after_delay(Some("600"), Some(429), cap),
            Some(Duration::from_secs(30))
        );
        // HTTP-date in the near future.
        let future = chrono::Utc::now() + chrono::Duration::seconds(10);
        let delay = RetryPolicy::retry_after_delay(Some(&future.to_rfc2822()), Some(503), cap)
            .expect("http-date form must parse");
        assert!(delay <= Duration::from_secs(10) && delay > Duration::from_secs(5));

        // Ignored for other statuses or bad input.
        assert_eq!(
            RetryPolicy::retry_after_delay(Some("5"), Some(500), cap),
            None
        );
        assert_eq!(
            RetryPolicy::retry_after_delay(Some("soon"), Some(503), cap),
            None
        );
        assert_eq!(RetryPolicy::retry_after_delay(None, Some(503), cap), None);
        assert_eq!(RetryPolicy::retry_after_delay(Some("5"), None, cap), None);
    }
}
