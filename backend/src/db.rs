//! SQLite pool setup and migrations.
//!
//! Migrations live in the workspace `migrations/` directory and run
//! automatically at startup (`sqlx::migrate!`). They are append-only —
//! startup never drops or rewrites existing data.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Creates the SQLite parent directory (when the URL points at a file),
/// opens a pooled connection with safety pragmas enabled, and applies
/// pending migrations.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    create_parent_dir(database_url);

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    sqlx::migrate!("../migrations").run(&pool).await?;
    Ok(pool)
}

fn create_parent_dir(database_url: &str) {
    let Some(path) = database_url.strip_prefix("sqlite://") else {
        return;
    };
    // In-memory databases have no parent directory.
    if path == ":memory:" || path.starts_with("file::memory:") {
        return;
    }
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_apply_to_a_fresh_database() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}/test.db", dir.path().display());
        let pool = connect(&url).await.expect("migrations must apply");

        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(tables.contains(&"monitors".to_string()));
        assert!(tables.contains(&"check_results".to_string()));
        assert!(tables.contains(&"incidents".to_string()));
        assert!(tables.contains(&"settings".to_string()));
        assert!(tables.contains(&"monitor_pauses".to_string()));

        // Foreign key enforcement must be on for the pooled connection.
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fk, 1);

        pool.close().await;
    }

    #[tokio::test]
    async fn migrations_are_idempotent_across_reconnects() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}/test.db", dir.path().display());
        let pool = connect(&url).await.unwrap();
        pool.close().await;
        // Re-opening an existing database must not fail or wipe data.
        let pool = connect(&url).await.expect("second run must succeed");
        pool.close().await;
    }
}
