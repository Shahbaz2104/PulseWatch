//! Alert payloads and outbox delivery rows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Incident transition that produced an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum AlertEvent {
    Opened,
    Resolved,
}

impl AlertEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertEvent::Opened => "opened",
            AlertEvent::Resolved => "resolved",
        }
    }
}

/// Delivery channel for an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum AlertChannel {
    Webhook,
    Email,
}

/// Stable JSON body sent to webhooks and used to render email messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPayload {
    pub event: AlertEvent,
    pub monitor: AlertMonitor,
    pub incident: AlertIncident,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMonitor {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertIncident {
    pub id: Uuid,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub failure_reason: Option<String>,
    pub duration_seconds: i64,
}

/// Pending delivery claimed by the alert worker.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AlertDelivery {
    pub id: Uuid,
    pub channel: AlertChannel,
    pub destination: String,
    pub payload: String,
    pub attempts: i64,
}
