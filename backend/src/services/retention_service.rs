//! Check-history retention.
//!
//! Old checks and completed alert deliveries are deleted in bounded batches
//! so a large purge never holds one long write transaction. Incidents are
//! never deleted: they are the durable outage record. The worker runs once at
//! startup (the interval's first tick fires immediately) and every six hours.

use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;

use crate::services::settings_service::AppSettings;
use crate::state::AppState;

/// How often the retention worker runs after its startup pass.
pub const PURGE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Rows deleted per statement; keeps SQLite write locks short.
const BATCH_SIZE: i64 = 1_000;

/// Rows removed by one retention pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PurgeStats {
    pub checks: u64,
    pub alert_deliveries: u64,
}

/// Delete rows older than the configured retention. Safe to call directly
/// from tests and from the background worker.
pub async fn purge_once(state: &AppState) -> Result<PurgeStats, sqlx::Error> {
    let settings = AppSettings::load(&state.pool, &state.config).await?;
    let cutoff = Utc::now() - chrono::Duration::days(settings.check_retention_days);
    let checks = purge_checks(state, cutoff).await?;
    let alert_deliveries = purge_alert_deliveries(state, cutoff).await?;
    Ok(PurgeStats {
        checks,
        alert_deliveries,
    })
}

async fn purge_checks(state: &AppState, cutoff: chrono::DateTime<Utc>) -> Result<u64, sqlx::Error> {
    let mut total = 0u64;
    loop {
        let result = sqlx::query(
            "DELETE FROM check_results WHERE rowid IN ( \
             SELECT rowid FROM check_results WHERE checked_at < ? \
             ORDER BY checked_at LIMIT ?)",
        )
        .bind(cutoff)
        .bind(BATCH_SIZE)
        .execute(&state.pool)
        .await?;
        total += result.rows_affected();
        if result.rows_affected() < BATCH_SIZE as u64 {
            break;
        }
    }
    Ok(total)
}

async fn purge_alert_deliveries(
    state: &AppState,
    cutoff: chrono::DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let mut total = 0u64;
    loop {
        let result = sqlx::query(
            "DELETE FROM alert_deliveries WHERE rowid IN ( \
             SELECT rowid FROM alert_deliveries \
             WHERE (status = 'delivered' AND COALESCE(delivered_at, created_at) < ?) \
                OR (status = 'failed' AND created_at < ?) \
             ORDER BY created_at LIMIT ?)",
        )
        .bind(cutoff)
        .bind(cutoff)
        .bind(BATCH_SIZE)
        .execute(&state.pool)
        .await?;
        total += result.rows_affected();
        if result.rows_affected() < BATCH_SIZE as u64 {
            break;
        }
    }
    Ok(total)
}

/// Spawn the retention loop. Stops when the shared shutdown token is
/// cancelled.
pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(PURGE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tracing::info!(
            interval_hours = PURGE_INTERVAL.as_secs() / 3_600,
            "retention worker started"
        );
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => {
                    tracing::info!("retention worker stopped");
                    break;
                }
                _ = ticker.tick() => {
                    match purge_once(&state).await {
                        Ok(stats) if stats.checks > 0 || stats.alert_deliveries > 0 => {
                            tracing::info!(
                                checks = stats.checks,
                                alert_deliveries = stats.alert_deliveries,
                                "retention purge completed"
                            );
                        }
                        Ok(_) => tracing::debug!("retention purge found nothing to delete"),
                        Err(err) => {
                            tracing::error!(error = %err, "retention purge failed");
                        }
                    }
                }
            }
        }
    })
}
