//! Key/value persistence for application settings overrides.
//!
//! The `settings` table stores string values; rows absent from the table fall
//! back to environment-derived config.

use std::collections::HashMap;

/// Read every stored override as a `key -> raw value` map.
pub async fn read_all(pool: &sqlx::SqlitePool) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}

/// Insert or update a single setting value.
pub async fn upsert(
    pool: &sqlx::SqlitePool,
    key: &str,
    value: i64,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value.to_string())
    .bind(updated_at)
    .execute(pool)
    .await?;
    Ok(())
}
