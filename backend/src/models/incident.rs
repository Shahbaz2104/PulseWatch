//! Incident entity — opened on an operational→down transition, resolved when
//! the monitor's recovery rule is satisfied.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum IncidentStatus {
    Open,
    Resolved,
}

impl IncidentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IncidentStatus::Open => "open",
            IncidentStatus::Resolved => "resolved",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(IncidentStatus::Open),
            "resolved" => Some(IncidentStatus::Resolved),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Incident {
    pub id: Uuid,
    pub monitor_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub status: IncidentStatus,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Incident joined with the affected monitor's identity for list views.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IncidentWithMonitor {
    #[serde(flatten)]
    pub incident: Incident,
    pub monitor_name: String,
    pub monitor_url: String,
    /// Duration in seconds; for open incidents this is elapsed time at query
    /// time. Never double-counts overlapping incidents (one open max per
    /// monitor is enforced by a unique index).
    pub duration_seconds: i64,
}

/// Query parameters for incident listing.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListIncidentsQuery {
    /// Filter by incident state: `open` or `resolved`.
    pub status: Option<String>,
    /// Restrict to a single monitor id.
    pub monitor_id: Option<Uuid>,
    /// RFC3339 window start (matches `started_at`).
    pub from: Option<String>,
    /// RFC3339 window end.
    pub to: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incident_status_round_trips() {
        assert_eq!(IncidentStatus::parse("open"), Some(IncidentStatus::Open));
        assert_eq!(
            IncidentStatus::parse("resolved"),
            Some(IncidentStatus::Resolved)
        );
        assert_eq!(IncidentStatus::parse("closed"), None);
        assert_eq!(serde_json::to_string(&IncidentStatus::Open).unwrap(), "\"open\"");
    }
}
