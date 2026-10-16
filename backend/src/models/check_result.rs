//! Check result entity and error classification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Classified failure reason for a completed check.
///
/// HTTP responses that do not match the expected status range are *not*
/// transport failures — they are received responses that failed the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum CheckErrorType {
    Dns,
    Connection,
    Timeout,
    Tls,
    HttpStatusMismatch,
    TooManyRedirects,
    BlockedTarget,
    Cancelled,
    Internal,
}

impl CheckErrorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckErrorType::Dns => "dns",
            CheckErrorType::Connection => "connection",
            CheckErrorType::Timeout => "timeout",
            CheckErrorType::Tls => "tls",
            CheckErrorType::HttpStatusMismatch => "http_status_mismatch",
            CheckErrorType::TooManyRedirects => "too_many_redirects",
            CheckErrorType::BlockedTarget => "blocked_target",
            CheckErrorType::Cancelled => "cancelled",
            CheckErrorType::Internal => "internal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "dns" => CheckErrorType::Dns,
            "connection" => CheckErrorType::Connection,
            "timeout" => CheckErrorType::Timeout,
            "tls" => CheckErrorType::Tls,
            "http_status_mismatch" => CheckErrorType::HttpStatusMismatch,
            "too_many_redirects" => CheckErrorType::TooManyRedirects,
            "blocked_target" => CheckErrorType::BlockedTarget,
            "cancelled" => CheckErrorType::Cancelled,
            "internal" => CheckErrorType::Internal,
            _ => return None,
        })
    }

    /// Whether a failed check with this error class is worth retrying.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            CheckErrorType::Dns
                | CheckErrorType::Connection
                | CheckErrorType::Timeout
                | CheckErrorType::Cancelled
        )
    }
}

/// Final outcome of one scheduled or manual check (one row per check,
/// after retries are exhausted or the first success arrives).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CheckResult {
    pub id: Uuid,
    pub monitor_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub http_status_code: Option<i64>,
    pub response_time_ms: Option<i64>,
    pub success: bool,
    pub error_type: Option<CheckErrorType>,
    pub error_message: Option<String>,
    pub attempt_count: i64,
    pub expected_status_match: bool,
}

/// Aggregated statistics for a set of checks inside a reporting window.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CheckSummary {
    pub total_checks: i64,
    pub successful_checks: i64,
    /// `null` when there are no eligible checks (never coerced to 100.0).
    pub uptime_percent: Option<f64>,
    pub latest_response_time_ms: Option<i64>,
    pub avg_response_time_ms: Option<f64>,
    pub median_response_time_ms: Option<f64>,
    pub p95_response_time_ms: Option<f64>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
}

/// A single historical check row returned by history endpoints.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CheckResultDto {
    #[serde(flatten)]
    pub check: CheckResult,
}

/// Query parameters for check history.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListChecksQuery {
    /// RFC3339 window start.
    pub from: Option<String>,
    /// RFC3339 window end.
    pub to: Option<String>,
    /// Filter by outcome: `success` or `failure`.
    pub result: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_type_round_trips() {
        for err in [
            CheckErrorType::Dns,
            CheckErrorType::Connection,
            CheckErrorType::Timeout,
            CheckErrorType::Tls,
            CheckErrorType::HttpStatusMismatch,
            CheckErrorType::TooManyRedirects,
            CheckErrorType::BlockedTarget,
            CheckErrorType::Cancelled,
            CheckErrorType::Internal,
        ] {
            assert_eq!(CheckErrorType::parse(err.as_str()), Some(err));
        }
    }

    #[test]
    fn transient_classification_is_stable() {
        assert!(CheckErrorType::Timeout.is_transient());
        assert!(CheckErrorType::Connection.is_transient());
        assert!(CheckErrorType::Dns.is_transient());
        assert!(!CheckErrorType::HttpStatusMismatch.is_transient());
        assert!(!CheckErrorType::Tls.is_transient());
        assert!(!CheckErrorType::BlockedTarget.is_transient());
    }
}
