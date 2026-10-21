//! Alert destination fan-out and the delivery outbox.

use chrono::{DateTime, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::models::alert::{
    AlertChannel, AlertDelivery, AlertEvent, AlertIncident, AlertMonitor, AlertPayload,
};
use crate::models::incident::Incident;
use crate::models::monitor::Monitor;

const INSERT_SQL: &str = "INSERT OR IGNORE INTO alert_deliveries \
(id, monitor_id, incident_id, event, channel, destination, payload, status, attempts, \
next_attempt_at, last_error, created_at, delivered_at) \
VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, NULL, ?, NULL)";

/// Queue one delivery for every destination configured on the monitor.
///
/// Runs inside the check transaction. `INSERT OR IGNORE` plus the unique
/// `(incident_id, event, channel)` index makes this idempotent.
pub async fn enqueue_for_transition(
    conn: &mut SqliteConnection,
    monitor: &Monitor,
    incident: &Incident,
    event: AlertEvent,
    occurred_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let payload = AlertPayload {
        event,
        monitor: AlertMonitor {
            id: monitor.id,
            name: monitor.name.clone(),
            url: monitor.url.clone(),
            status: incident.status.as_str().to_string(),
        },
        incident: AlertIncident {
            id: incident.id,
            status: incident.status.as_str().to_string(),
            started_at: incident.started_at,
            resolved_at: incident.resolved_at,
            failure_reason: incident.failure_reason.clone(),
            duration_seconds: incident
                .resolved_at
                .unwrap_or(occurred_at)
                .signed_duration_since(incident.started_at)
                .num_seconds()
                .max(0),
        },
        occurred_at,
    };
    let payload = serde_json::to_string(&payload)
        .map_err(|err| sqlx::Error::Protocol(format!("serialize alert payload: {err}")))?;

    if let Some(destination) = monitor.alert_webhook_url.as_deref() {
        insert(
            conn,
            monitor.id,
            incident.id,
            event,
            AlertChannel::Webhook,
            destination,
            &payload,
            occurred_at,
        )
        .await?;
    }
    if let Some(destination) = monitor.alert_email.as_deref() {
        insert(
            conn,
            monitor.id,
            incident.id,
            event,
            AlertChannel::Email,
            destination,
            &payload,
            occurred_at,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert(
    conn: &mut SqliteConnection,
    monitor_id: Uuid,
    incident_id: Uuid,
    event: AlertEvent,
    channel: AlertChannel,
    destination: &str,
    payload: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT_SQL)
        .bind(Uuid::new_v4())
        .bind(monitor_id)
        .bind(incident_id)
        .bind(event)
        .bind(channel)
        .bind(destination)
        .bind(payload)
        .bind(now)
        .bind(now)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Due pending deliveries, oldest first.
pub async fn due(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<AlertDelivery>, sqlx::Error> {
    sqlx::query_as::<_, AlertDelivery>(
        "SELECT id, channel, destination, payload, attempts FROM alert_deliveries \
         WHERE status = 'pending' AND next_attempt_at <= ? \
         ORDER BY next_attempt_at, id LIMIT ?",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn mark_delivered(
    pool: &SqlitePool,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE alert_deliveries SET status = 'delivered', delivered_at = ?, last_error = NULL \
         WHERE id = ?",
    )
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_retry(
    pool: &SqlitePool,
    id: Uuid,
    attempts: i64,
    next_attempt_at: DateTime<Utc>,
    last_error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE alert_deliveries SET attempts = ?, next_attempt_at = ?, last_error = ? \
         WHERE id = ?",
    )
    .bind(attempts)
    .bind(next_attempt_at)
    .bind(truncate_error(last_error))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(
    pool: &SqlitePool,
    id: Uuid,
    attempts: i64,
    last_error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE alert_deliveries SET status = 'failed', attempts = ?, last_error = ? WHERE id = ?",
    )
    .bind(attempts)
    .bind(truncate_error(last_error))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

fn truncate_error(error: &str) -> String {
    const MAX: usize = 500;
    let trimmed = error.trim();
    if trimmed.len() <= MAX {
        return trimmed.to_string();
    }
    let mut end = MAX;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_bounded() {
        let message = "x".repeat(900);
        assert_eq!(truncate_error(&message).len(), 500);
    }
}
