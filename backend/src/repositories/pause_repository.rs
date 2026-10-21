//! Pause interval persistence.
//!
//! A row is opened when a monitor is paused and closed exactly once on
//! resume. The partial unique index `idx_monitor_pauses_one_open` guarantees
//! at most one open interval per monitor.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, Sqlite};
use uuid::Uuid;

/// One paused interval, as needed by pause/resume and report clipping.
#[derive(Debug, Clone, FromRow)]
pub struct PauseInterval {
    pub monitor_id: Uuid,
    pub paused_at: DateTime<Utc>,
    pub resumed_at: Option<DateTime<Utc>>,
}

/// Open a pause interval. Fails if one is already open for the monitor.
pub async fn open<'c, E>(
    executor: E,
    monitor_id: Uuid,
    paused_at: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO monitor_pauses (id, monitor_id, paused_at, resumed_at) \
         VALUES (?, ?, ?, NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_id)
    .bind(paused_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Close the open interval (if any). Returns whether a row was closed.
pub async fn close_open<'c, E>(
    executor: E,
    monitor_id: Uuid,
    resumed_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE monitor_pauses SET resumed_at = ? \
         WHERE monitor_id = ? AND resumed_at IS NULL",
    )
    .bind(resumed_at)
    .bind(monitor_id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Pause intervals overlapping `[start, end)`, including intervals that began
/// before the window and open intervals that have not resumed yet.
pub async fn window_intervals(
    pool: &sqlx::SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<PauseInterval>, sqlx::Error> {
    sqlx::query_as::<_, PauseInterval>(
        "SELECT monitor_id, paused_at, resumed_at FROM monitor_pauses \
         WHERE paused_at < ? AND (resumed_at IS NULL OR resumed_at > ?) \
         ORDER BY paused_at",
    )
    .bind(end)
    .bind(start)
    .fetch_all(pool)
    .await
}
