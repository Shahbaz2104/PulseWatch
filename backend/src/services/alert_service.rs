//! Incident alert delivery: webhooks and email through a durable outbox.
//!
//! Transitions queue delivery rows in the same transaction as the check. This
//! worker claims due rows, sends them with bounded timeouts, and retries
//! transient failures with exponential backoff.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lettre::message::Message;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::{Config, SmtpConfig, SmtpTls};
use crate::models::alert::{AlertChannel, AlertDelivery, AlertEvent, AlertPayload};
use crate::repositories::alert_repository;
use crate::state::AppState;
use crate::validation::{validate_monitor_url, SsrfPolicy};

/// Attempts include the first send plus retries.
pub const MAX_ATTEMPTS: i64 = 5;
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);
const EMAIL_TIMEOUT: Duration = Duration::from_secs(15);
const BATCH_SIZE: i64 = 20;

/// Sends a rendered email. Implemented by SMTP in production and by a fake in
/// tests.
#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}

/// SMTP-backed sender built from environment configuration.
pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SmtpEmailSender {
    pub fn from_config(config: &SmtpConfig) -> Result<Self, String> {
        let mut builder = match config.tls {
            SmtpTls::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                .map_err(|err| format!("invalid SMTP STARTTLS host: {err}"))?,
            SmtpTls::Implicit => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                .map_err(|err| format!("invalid SMTP implicit-TLS host: {err}"))?,
            SmtpTls::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
        }
        .port(config.port);
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
        }
        Ok(Self {
            transport: builder.build(),
            from: config.from.clone(),
        })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        let email = Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|err| format!("invalid SMTP_FROM: {err}"))?,
            )
            .to(to
                .parse()
                .map_err(|err| format!("invalid alert recipient: {err}"))?)
            .subject(subject)
            .body(body.to_string())
            .map_err(|err| format!("build email: {err}"))?;
        self.transport
            .send(email)
            .await
            .map(|_| ())
            .map_err(|err| format!("SMTP delivery failed: {err}"))
    }
}

/// Spawn the background delivery loop. Email is optional; webhooks still work
/// when SMTP is not configured.
pub fn spawn(state: AppState) -> JoinHandle<()> {
    let sender = build_sender(&state.config);
    tokio::spawn(async move {
        let email = sender.as_ref().map(|sender| sender as &dyn EmailSender);
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = ticker.tick() => {
                    if let Err(err) = deliver_due_once(&state, email).await {
                        tracing::error!(error = %err, "alert worker tick failed");
                    }
                }
            }
        }
        tracing::info!("alert worker stopped");
    })
}

fn build_sender(config: &Config) -> Option<SmtpEmailSender> {
    let smtp = config.smtp.as_ref()?;
    match SmtpEmailSender::from_config(smtp) {
        Ok(sender) => {
            tracing::info!(host = %smtp.host, port = smtp.port, "email alerts enabled");
            Some(sender)
        }
        Err(err) => {
            tracing::error!(error = %err, "email alerts disabled");
            None
        }
    }
}

/// Process one batch of due deliveries. Public for integration tests.
pub async fn deliver_due_once(
    state: &AppState,
    email: Option<&dyn EmailSender>,
) -> Result<usize, sqlx::Error> {
    let now = Utc::now();
    let due = alert_repository::due(&state.pool, now, BATCH_SIZE).await?;
    let count = due.len();
    for delivery in due {
        process_delivery(state, email, delivery, now).await?;
    }
    Ok(count)
}

async fn process_delivery(
    state: &AppState,
    email: Option<&dyn EmailSender>,
    delivery: AlertDelivery,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let attempts = delivery.attempts + 1;
    let result = match delivery.channel {
        AlertChannel::Webhook => send_webhook(state, &delivery).await,
        AlertChannel::Email => send_email(email, &delivery).await,
    };

    match result {
        DeliveryResult::Delivered => {
            alert_repository::mark_delivered(&state.pool, delivery.id, now).await?;
            tracing::info!(delivery_id = %delivery.id, channel = ?delivery.channel, "alert delivered");
        }
        DeliveryResult::Retry(message) => {
            if attempts >= MAX_ATTEMPTS {
                alert_repository::mark_failed(&state.pool, delivery.id, attempts, &message).await?;
                tracing::warn!(
                    delivery_id = %delivery.id,
                    attempts,
                    error = %message,
                    "alert delivery exhausted retries"
                );
            } else {
                let next = now + chrono::Duration::seconds(retry_delay_seconds(attempts));
                alert_repository::mark_retry(&state.pool, delivery.id, attempts, next, &message)
                    .await?;
                tracing::warn!(
                    delivery_id = %delivery.id,
                    attempts,
                    retry_at = %next,
                    error = %message,
                    "alert delivery failed; will retry"
                );
            }
        }
        DeliveryResult::Permanent(message) => {
            alert_repository::mark_failed(&state.pool, delivery.id, attempts, &message).await?;
            tracing::warn!(
                delivery_id = %delivery.id,
                error = %message,
                "alert delivery failed permanently"
            );
        }
    }
    Ok(())
}

enum DeliveryResult {
    Delivered,
    /// Transient failure; try again until `MAX_ATTEMPTS`.
    Retry(String),
    Permanent(String),
}

async fn send_webhook(state: &AppState, delivery: &AlertDelivery) -> DeliveryResult {
    if let Err(err) = validate_monitor_url(
        &delivery.destination,
        &SsrfPolicy::from_config(&state.config),
    ) {
        return DeliveryResult::Permanent(format!("webhook destination rejected: {err}"));
    }

    let request = state
        .http_client
        .post(&delivery.destination)
        .header("content-type", "application/json")
        .header(
            "user-agent",
            concat!("PulseWatch/", env!("CARGO_PKG_VERSION")),
        )
        .body(delivery.payload.clone());

    match tokio::time::timeout(WEBHOOK_TIMEOUT, request.send()).await {
        Err(_) => DeliveryResult::Retry("webhook timed out".to_string()),
        Ok(Err(err)) => DeliveryResult::Retry(format!("webhook request failed: {err}")),
        Ok(Ok(response)) if response.status().is_success() => DeliveryResult::Delivered,
        Ok(Ok(response)) => {
            let status = response.status();
            let message = format!("webhook returned HTTP {status}");
            if status.as_u16() == 408
                || status.as_u16() == 425
                || status.as_u16() == 429
                || status.is_server_error()
            {
                DeliveryResult::Retry(message)
            } else {
                DeliveryResult::Permanent(message)
            }
        }
    }
}

async fn send_email(sender: Option<&dyn EmailSender>, delivery: &AlertDelivery) -> DeliveryResult {
    let Some(sender) = sender else {
        return DeliveryResult::Permanent("SMTP is not configured".to_string());
    };
    let payload: AlertPayload = match serde_json::from_str(&delivery.payload) {
        Ok(payload) => payload,
        Err(err) => return DeliveryResult::Permanent(format!("invalid alert payload: {err}")),
    };
    let (subject, body) = render_email(&payload);
    match tokio::time::timeout(
        EMAIL_TIMEOUT,
        sender.send(&delivery.destination, &subject, &body),
    )
    .await
    {
        Err(_) => DeliveryResult::Retry("email delivery timed out".to_string()),
        Ok(Err(err)) => DeliveryResult::Retry(err),
        Ok(Ok(())) => DeliveryResult::Delivered,
    }
}

/// Exponential retry delay: 30s, 60s, 120s, 240s (capped at one hour).
pub fn retry_delay_seconds(attempts: i64) -> i64 {
    let exponent = attempts.saturating_sub(1).min(10) as u32;
    (30_i64 * 2_i64.pow(exponent)).min(3600)
}

pub fn render_email(payload: &AlertPayload) -> (String, String) {
    let (action, verb) = match payload.event {
        AlertEvent::Opened => ("DOWN", "is down"),
        AlertEvent::Resolved => ("RECOVERED", "has recovered"),
    };
    let subject = format!("[PulseWatch] {action}: {}", payload.monitor.name);
    let resolved = payload
        .incident
        .resolved_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "still open".to_string());
    let reason = payload
        .incident
        .failure_reason
        .as_deref()
        .unwrap_or("(none recorded)");
    let body = format!(
        "PulseWatch alert: {} {verb}\n\
         \n\
         Monitor:  {} ({})\n\
         Status:   {}\n\
         Incident: {}\n\
         Started:  {}\n\
         Resolved: {}\n\
         Duration: {}s\n\
         Reason:   {}\n",
        payload.monitor.name,
        payload.monitor.name,
        payload.monitor.url,
        payload.monitor.status,
        payload.incident.id,
        payload.incident.started_at.to_rfc3339(),
        resolved,
        payload.incident.duration_seconds,
        reason,
    );
    (subject, body)
}

/// Cancellation-aware sleep used by tests that need to stop the worker early.
pub async fn wait_for_shutdown(token: &CancellationToken) {
    token.cancelled().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::alert::{AlertIncident, AlertMonitor};
    use uuid::Uuid;

    fn payload(event: AlertEvent) -> AlertPayload {
        let now = Utc::now();
        AlertPayload {
            event,
            monitor: AlertMonitor {
                id: Uuid::new_v4(),
                name: "API".into(),
                url: "https://example.com/health".into(),
                status: if event == AlertEvent::Opened {
                    "open".into()
                } else {
                    "resolved".into()
                },
            },
            incident: AlertIncident {
                id: Uuid::new_v4(),
                status: "open".into(),
                started_at: now,
                resolved_at: (event == AlertEvent::Resolved).then_some(now),
                failure_reason: Some("timeout: no response".into()),
                duration_seconds: 42,
            },
            occurred_at: now,
        }
    }

    #[test]
    fn retry_delays_grow_then_cap() {
        assert_eq!(retry_delay_seconds(1), 30);
        assert_eq!(retry_delay_seconds(2), 60);
        assert_eq!(retry_delay_seconds(3), 120);
        assert_eq!(retry_delay_seconds(4), 240);
        assert_eq!(retry_delay_seconds(20), 3600);
    }

    #[test]
    fn emails_are_human_readable() {
        let (subject, body) = render_email(&payload(AlertEvent::Opened));
        assert_eq!(subject, "[PulseWatch] DOWN: API");
        assert!(body.contains("https://example.com/health"));
        assert!(body.contains("timeout"));
        let (subject, _) = render_email(&payload(AlertEvent::Resolved));
        assert_eq!(subject, "[PulseWatch] RECOVERED: API");
    }
}
