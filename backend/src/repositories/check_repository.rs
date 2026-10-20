//! Check-result persistence.

use chrono::{DateTime, Utc};
use sqlx::Sqlite;
use uuid::Uuid;

use crate::models::check_result::{CheckResult, LatencyStats};

// Literal SQL: sqlx 0.9 accepts only `&'static str` (or explicitly asserted
// safe strings) for query text.
const INSERT_SQL: &str = "INSERT INTO check_results (id, monitor_id, checked_at, completed_at, \
http_status_code, response_time_ms, success, error_type, error_message, attempt_count, \
expected_status_match) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

const SELECT_COLUMNS: &str = "id, monitor_id, checked_at, completed_at, http_status_code, \
response_time_ms, success, error_type, error_message, attempt_count, expected_status_match";

const LATEST_SQL: &str = "SELECT id, monitor_id, checked_at, completed_at, http_status_code, \
response_time_ms, success, error_type, error_message, attempt_count, expected_status_match \
FROM check_results WHERE monitor_id = ? ORDER BY checked_at DESC, id DESC LIMIT 1";

pub async fn insert<'c, E>(executor: E, result: &CheckResult) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query(INSERT_SQL)
        .bind(result.id)
        .bind(result.monitor_id)
        .bind(result.checked_at)
        .bind(result.completed_at)
        .bind(result.http_status_code)
        .bind(result.response_time_ms)
        .bind(result.success)
        .bind(result.error_type)
        .bind(&result.error_message)
        .bind(result.attempt_count)
        .bind(result.expected_status_match)
        .execute(executor)
        .await?;
    Ok(())
}

/// Filters for check history queries.
#[derive(Debug, Clone, Default)]
pub struct CheckFilter {
    /// Inclusive window start.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive window end.
    pub to: Option<DateTime<Utc>>,
    /// `Some(true)` for successes, `Some(false)` for failures.
    pub success: Option<bool>,
    pub page: i64,
    pub page_size: i64,
}

impl CheckFilter {
    pub fn page(&self) -> i64 {
        self.page.max(1)
    }

    pub fn page_size(&self) -> i64 {
        self.page_size.clamp(1, 200)
    }
}

/// Historical checks for one monitor, newest first, with pagination and
/// optional window/outcome filters. Returns the page and the total count.
pub async fn list_for_monitor(
    pool: &sqlx::SqlitePool,
    monitor_id: Uuid,
    filter: &CheckFilter,
) -> Result<(Vec<CheckResult>, i64), sqlx::Error> {
    let build_where = |query: &mut sqlx::QueryBuilder<Sqlite>| {
        // Placeholders must come from `push_bind`; `?` inside `push()` is
        // emitted literally by sqlx 0.9 and breaks the prepared statement.
        query.push(" WHERE monitor_id = ");
        query.push_bind(monitor_id);
        if let Some(from) = filter.from {
            query.push(" AND checked_at >= ");
            query.push_bind(from);
        }
        if let Some(to) = filter.to {
            query.push(" AND checked_at < ");
            query.push_bind(to);
        }
        if let Some(success) = filter.success {
            query.push(" AND success = ");
            query.push_bind(success);
        }
    };

    let mut count_q = sqlx::QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM check_results");
    build_where(&mut count_q);
    let total: i64 = count_q.build_query_scalar().fetch_one(pool).await?;

    let mut page_q =
        sqlx::QueryBuilder::<Sqlite>::new(format!("SELECT {SELECT_COLUMNS} FROM check_results"));
    build_where(&mut page_q);
    page_q.push(" ORDER BY checked_at DESC, id DESC LIMIT ");
    page_q.push_bind(filter.page_size());
    page_q.push(" OFFSET ");
    page_q.push_bind((filter.page() - 1) * filter.page_size());

    let rows = page_q
        .build_query_as::<CheckResult>()
        .fetch_all(pool)
        .await?;
    Ok((rows, total))
}

/// Most recent completed check for a monitor.
pub async fn latest_for_monitor<'c, E>(
    executor: E,
    monitor_id: Uuid,
) -> Result<Option<CheckResult>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query_as::<_, CheckResult>(LATEST_SQL)
        .bind(monitor_id)
        .fetch_optional(executor)
        .await
}

// ---------------------------------------------------------------------------
// Report aggregations
// ---------------------------------------------------------------------------

/// Per-monitor check counts and successes inside `[start, end)`.
pub async fn window_counts(
    pool: &sqlx::SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<(Uuid, i64, i64)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, i64, i64)>(
        "SELECT monitor_id, COUNT(*), SUM(CASE WHEN success THEN 1 ELSE 0 END) \
         FROM check_results WHERE checked_at >= ? AND checked_at < ? \
         GROUP BY monitor_id",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Successful checks with a recorded response time (the latency sample set).
async fn latency_sample_count(
    pool: &sqlx::SqlitePool,
    monitor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM check_results \
         WHERE monitor_id = ? AND checked_at >= ? AND checked_at < ? \
         AND success = 1 AND response_time_ms IS NOT NULL",
    )
    .bind(monitor_id)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await
}

/// The latency sample at a 0-based rank offset (samples ordered ascending).
async fn latency_sample_at(
    pool: &sqlx::SqlitePool,
    monitor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    offset: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT response_time_ms FROM check_results \
         WHERE monitor_id = ? AND checked_at >= ? AND checked_at < ? \
         AND success = 1 AND response_time_ms IS NOT NULL \
         ORDER BY response_time_ms LIMIT 1 OFFSET ?",
    )
    .bind(monitor_id)
    .bind(start)
    .bind(end)
    .bind(offset)
    .fetch_optional(pool)
    .await
}

/// Latency aggregates for one monitor: sample count, average, and exact
/// nearest-rank median/p95 (SQLite has no percentile functions, so the value
/// at the computed offset is fetched directly — no full result set in memory).
///
/// Returns `(samples, avg, median, p95)`; median/p95 are `None` when fewer
/// than 2 samples exist.
pub async fn latency_stats(
    pool: &sqlx::SqlitePool,
    monitor_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<(i64, Option<f64>, Option<i64>, Option<i64>), sqlx::Error> {
    let avg: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(response_time_ms) FROM check_results \
         WHERE monitor_id = ? AND checked_at >= ? AND checked_at < ? \
         AND success = 1 AND response_time_ms IS NOT NULL",
    )
    .bind(monitor_id)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await?;

    let samples = latency_sample_count(pool, monitor_id, start, end).await?;
    if samples < 2 {
        return Ok((samples, avg, None, None));
    }
    let median = latency_sample_at(pool, monitor_id, start, end, (samples - 1) / 2).await?;
    let p95_offset = crate::services::report_service::nearest_rank_offset(0.95, samples);
    let p95 = latency_sample_at(pool, monitor_id, start, end, p95_offset).await?;
    Ok((samples, avg, median, p95))
}

/// Latency aggregates for every monitor in one pass: sample count, average,
/// and exact nearest-rank median/p95 per monitor.
///
/// Replaces per-monitor round trips (avg + count + two OFFSET fetches each)
/// with a single scan over the `(checked_at)` index, grouped and ranked in
/// memory. Monitors with no successful checks in the window are absent from
/// the map.
pub async fn latency_stats_batch(
    pool: &sqlx::SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<std::collections::HashMap<Uuid, LatencyStats>, sqlx::Error> {
    // Rows arrive ordered by (monitor_id, response_time_ms), so each
    // monitor's samples are contiguous and sorted ascending.
    let rows: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT monitor_id, response_time_ms FROM check_results \
         WHERE checked_at >= ? AND checked_at < ? \
         AND success = 1 AND response_time_ms IS NOT NULL \
         ORDER BY monitor_id, response_time_ms",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    let mut grouped: std::collections::HashMap<Uuid, Vec<i64>> = std::collections::HashMap::new();
    for (monitor_id, response_time_ms) in rows {
        grouped
            .entry(monitor_id)
            .or_default()
            .push(response_time_ms);
    }

    Ok(grouped
        .into_iter()
        .map(|(monitor_id, samples_sorted)| {
            let n = samples_sorted.len() as i64;
            let avg = if n > 0 {
                Some(samples_sorted.iter().sum::<i64>() as f64 / n as f64)
            } else {
                None
            };
            let (median, p95) = if n < 2 {
                (None, None)
            } else {
                let median_index = ((n - 1) / 2) as usize;
                let p95_index =
                    crate::services::report_service::nearest_rank_offset(0.95, n) as usize;
                (
                    Some(samples_sorted[median_index]),
                    Some(samples_sorted[p95_index]),
                )
            };
            (
                monitor_id,
                LatencyStats {
                    samples: n,
                    avg,
                    median,
                    p95,
                },
            )
        })
        .collect())
}

/// Time-bucketed activity (hourly or daily, UTC-aligned via SQLite strftime).
/// `monitor_id = None` aggregates across all monitors. Buckets come back as
/// RFC3339 strings (`...T00:00:00Z` / `...T13:00:00Z`) for the service to
/// parse; empty ranges simply yield no rows.
pub async fn activity_series(
    pool: &sqlx::SqlitePool,
    monitor_id: Option<Uuid>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    daily: bool,
) -> Result<Vec<(String, i64, i64, Option<f64>)>, sqlx::Error> {
    let fmt = if daily {
        "%Y-%m-%dT00:00:00Z"
    } else {
        "%Y-%m-%dT%H:00:00Z"
    };
    let mut query = sqlx::QueryBuilder::<Sqlite>::new("SELECT strftime('");
    query.push(fmt);
    query.push(
        "', c.checked_at) AS bucket, COUNT(*), \
               SUM(CASE WHEN c.success THEN 1 ELSE 0 END), \
               AVG(c.response_time_ms) FROM check_results c WHERE c.checked_at >= ",
    );
    query.push_bind(start);
    query.push(" AND c.checked_at < ");
    query.push_bind(end);
    if let Some(id) = monitor_id {
        query.push(" AND c.monitor_id = ");
        query.push_bind(id);
    }
    query.push(" GROUP BY bucket ORDER BY bucket");

    query
        .build_query_as::<(String, i64, i64, Option<f64>)>()
        .fetch_all(pool)
        .await
}

/// Newest checks across all monitors inside `[start, end)` (overview feed).
pub async fn recent_global(
    pool: &sqlx::SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<(Uuid, String, DateTime<Utc>, bool, Option<i64>, Option<i64>)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String, DateTime<Utc>, bool, Option<i64>, Option<i64>)>(
        "SELECT c.monitor_id, m.name, c.checked_at, c.success, c.http_status_code, \
         c.response_time_ms FROM check_results c JOIN monitors m ON m.id = c.monitor_id \
         WHERE c.checked_at >= ? AND c.checked_at < ? \
         ORDER BY c.checked_at DESC, c.id DESC LIMIT ?",
    )
    .bind(start)
    .bind(end)
    .bind(limit)
    .fetch_all(pool)
    .await
}
