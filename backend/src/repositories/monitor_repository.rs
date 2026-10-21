//! Monitor persistence: CRUD, listing with filters, scheduling helpers and
//! the aggregate lookups used to enrich list responses.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::Sqlite;
use uuid::Uuid;

use crate::models::monitor::{ListMonitorsQuery, Monitor};

// Full column list kept in literal SQL (sqlx 0.9 requires `&'static str` or an
// explicitly asserted safe string; literals also make injection impossible).
const INSERT_SQL: &str = "INSERT INTO monitors (id, name, url, enabled, check_interval_seconds, \
timeout_ms, expected_status_min, expected_status_max, consecutive_failures_threshold, \
consecutive_successes_threshold, retry_max_attempts, current_status, consecutive_failures, \
consecutive_successes, alert_webhook_url, alert_email, created_at, updated_at, \
last_checked_at, next_check_at) \
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

const SELECT_COLUMNS: &str = "id, name, url, enabled, check_interval_seconds, timeout_ms, \
expected_status_min, expected_status_max, consecutive_failures_threshold, \
consecutive_successes_threshold, retry_max_attempts, alert_webhook_url, \
alert_email, current_status, consecutive_failures, consecutive_successes, \
created_at, updated_at, last_checked_at, next_check_at";

const SELECT_BY_ID_SQL: &str =
    "SELECT id, name, url, enabled, check_interval_seconds, timeout_ms, \
expected_status_min, expected_status_max, consecutive_failures_threshold, \
consecutive_successes_threshold, retry_max_attempts, alert_webhook_url, \
alert_email, current_status, consecutive_failures, consecutive_successes, \
created_at, updated_at, last_checked_at, next_check_at FROM monitors WHERE id = ?";

pub async fn insert<'c, E>(executor: E, monitor: &Monitor) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query(INSERT_SQL)
        .bind(monitor.id)
        .bind(&monitor.name)
        .bind(&monitor.url)
        .bind(monitor.enabled)
        .bind(monitor.check_interval_seconds)
        .bind(monitor.timeout_ms)
        .bind(monitor.expected_status_min)
        .bind(monitor.expected_status_max)
        .bind(monitor.consecutive_failures_threshold)
        .bind(monitor.consecutive_successes_threshold)
        .bind(monitor.retry_max_attempts)
        .bind(monitor.current_status)
        .bind(monitor.consecutive_failures)
        .bind(monitor.consecutive_successes)
        .bind(&monitor.alert_webhook_url)
        .bind(&monitor.alert_email)
        .bind(monitor.created_at)
        .bind(monitor.updated_at)
        .bind(monitor.last_checked_at)
        .bind(monitor.next_check_at)
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn find_by_id<'c, E>(executor: E, id: Uuid) -> Result<Option<Monitor>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query_as::<_, Monitor>(SELECT_BY_ID_SQL)
        .bind(id)
        .fetch_optional(executor)
        .await
}

/// Update every editable column of an existing monitor.
pub async fn update<'c, E>(executor: E, monitor: &Monitor) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE monitors SET name = ?, url = ?, check_interval_seconds = ?, timeout_ms = ?, \
         expected_status_min = ?, expected_status_max = ?, consecutive_failures_threshold = ?, \
         consecutive_successes_threshold = ?, retry_max_attempts = ?, \
         alert_webhook_url = ?, alert_email = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(&monitor.name)
    .bind(&monitor.url)
    .bind(monitor.check_interval_seconds)
    .bind(monitor.timeout_ms)
    .bind(monitor.expected_status_min)
    .bind(monitor.expected_status_max)
    .bind(monitor.consecutive_failures_threshold)
    .bind(monitor.consecutive_successes_threshold)
    .bind(monitor.retry_max_attempts)
    .bind(&monitor.alert_webhook_url)
    .bind(&monitor.alert_email)
    .bind(monitor.updated_at)
    .bind(monitor.id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete<'c, E>(executor: E, id: Uuid) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let result = sqlx::query("DELETE FROM monitors WHERE id = ?")
        .bind(id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Pause: disable scheduling and mark the monitor `paused`.
pub async fn pause<'c, E>(executor: E, id: Uuid, now: DateTime<Utc>) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE monitors SET enabled = 0, current_status = 'paused', next_check_at = NULL, \
         updated_at = ? WHERE id = ? AND enabled = 1",
    )
    .bind(now)
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Resume: re-enable scheduling; status resets to `unknown` until the next
/// check confirms reality (a paused monitor must never be reported as up).
pub async fn resume<'c, E>(
    executor: E,
    id: Uuid,
    now: DateTime<Utc>,
    next_check_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let result = sqlx::query(
        "UPDATE monitors SET enabled = 1, current_status = 'unknown', consecutive_failures = 0, \
         consecutive_successes = 0, next_check_at = ?, updated_at = ? \
         WHERE id = ? AND enabled = 0",
    )
    .bind(next_check_at)
    .bind(now)
    .bind(id)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Apply a check outcome to the monitor row: counters, status, last check
/// time and the next scheduled check. Runs inside the caller's transaction.
pub async fn apply_check_outcome<'c, E>(
    executor: E,
    monitor_id: Uuid,
    status: crate::models::monitor::MonitorStatus,
    consecutive_failures: i64,
    consecutive_successes: i64,
    checked_at: DateTime<Utc>,
    next_check_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query(
        "UPDATE monitors SET current_status = ?, consecutive_failures = ?, \
         consecutive_successes = ?, last_checked_at = ?, updated_at = ?, next_check_at = ? \
         WHERE id = ?",
    )
    .bind(status)
    .bind(consecutive_failures)
    .bind(consecutive_successes)
    .bind(checked_at)
    .bind(checked_at)
    .bind(next_check_at)
    .bind(monitor_id)
    .execute(executor)
    .await?;
    Ok(())
}

/// Atomically claim due monitors for checking: each claimed row gets
/// `next_check_at = now + interval` (measured from claim time, so overdue
/// monitors after a restart are checked once — no catch-up burst). The
/// `enabled = 1 AND next_check_at <= now` guard in the UPDATE means only one
/// caller can win a given monitor.
pub async fn claim_due(
    pool: &sqlx::SqlitePool,
    now: DateTime<Utc>,
    max_results: i64,
) -> Result<Vec<Monitor>, sqlx::Error> {
    let due: Vec<(Uuid, i64)> = sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT id, check_interval_seconds FROM monitors \
         WHERE enabled = 1 AND next_check_at IS NOT NULL AND next_check_at <= ? \
         ORDER BY next_check_at LIMIT ?",
    )
    .bind(now)
    .bind(max_results)
    .fetch_all(pool)
    .await?;

    let mut claimed = Vec::with_capacity(due.len());
    for (id, interval) in due {
        let next = now + chrono::Duration::seconds(interval);
        let result = sqlx::query(
            "UPDATE monitors SET next_check_at = ? \
             WHERE id = ? AND enabled = 1 AND next_check_at IS NOT NULL AND next_check_at <= ?",
        )
        .bind(next)
        .bind(id)
        .bind(now)
        .execute(pool)
        .await?;
        if result.rows_affected() == 1 {
            if let Some(monitor) = find_by_id(pool, id).await? {
                claimed.push(monitor);
            }
        }
    }
    Ok(claimed)
}

/// One row of the windowed aggregate query.
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct WindowStats {
    pub checks: i64,
    pub successes: i64,
    pub avg_response_time_ms: Option<f64>,
}

/// Latest completed check snapshot per monitor.
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct LatestCheck {
    pub http_status_code: Option<i64>,
    pub response_time_ms: Option<i64>,
    pub success: bool,
    pub checked_at: DateTime<Utc>,
}

/// Pushes `(?, ?, ...)` placeholders for a list of UUIDs.
///
/// `QueryBuilder::push_values` emits SQLite `IN (VALUES ...)` syntax that
/// fails to prepare on our SQLite build, so placeholders are written by hand.
fn push_id_list(query: &mut sqlx::QueryBuilder<Sqlite>, ids: &[Uuid]) {
    query.push("(");
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            query.push(",");
        }
        query.push_bind(*id);
    }
    query.push(")");
}

/// Uptime/response aggregates for the given monitors inside `[start, end)`.
pub async fn window_stats<'c, E>(
    executor: E,
    ids: &[Uuid],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<HashMap<Uuid, WindowStats>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let mut out = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let mut query = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT monitor_id, COUNT(*) AS checks, \
         SUM(CASE WHEN success THEN 1 ELSE 0 END) AS successes, \
         AVG(response_time_ms) AS avg_response_time_ms \
         FROM check_results WHERE monitor_id IN ",
    );
    push_id_list(&mut query, ids);
    // Note: sqlx 0.9 `QueryBuilder` placeholders come from `push_bind` alone;
    // `?` characters inside `push()` strings are emitted literally.
    query.push(" AND checked_at >= ");
    query.push_bind(start);
    query.push(" AND checked_at < ");
    query.push_bind(end);
    query.push(" GROUP BY monitor_id");

    let rows = query
        .build_query_as::<(Uuid, i64, i64, Option<f64>)>()
        .fetch_all(executor)
        .await?;
    for (id, checks, successes, avg) in rows {
        out.insert(
            id,
            WindowStats {
                checks,
                successes,
                avg_response_time_ms: avg,
            },
        );
    }
    Ok(out)
}

/// Most recent check per monitor (any time).
pub async fn latest_checks<'c, E>(
    executor: E,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, LatestCheck>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let mut out = HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let mut query = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT c.monitor_id, c.http_status_code, c.response_time_ms, c.success, c.checked_at \
         FROM check_results c \
         JOIN (SELECT monitor_id, MAX(checked_at) AS mx FROM check_results WHERE monitor_id IN ",
    );
    push_id_list(&mut query, ids);
    query.push(
        " GROUP BY monitor_id) latest \
                ON latest.monitor_id = c.monitor_id AND latest.mx = c.checked_at",
    );

    let rows = query
        .build_query_as::<(Uuid, Option<i64>, Option<i64>, bool, DateTime<Utc>)>()
        .fetch_all(executor)
        .await?;
    for (id, http_status_code, response_time_ms, success, checked_at) in rows {
        out.insert(
            id,
            LatestCheck {
                http_status_code,
                response_time_ms,
                success,
                checked_at,
            },
        );
    }
    Ok(out)
}

/// Sort expressions allowed by the list endpoint (whitelist — user input only
/// selects between these; it is never interpolated as SQL). The uptime
/// expression contains one `?` placeholder for the window start.
fn sort_expression(sort: &str) -> &'static str {
    match sort {
        "name" => "m.name",
        "last_checked" => "m.last_checked_at",
        "created" => "m.created_at",
        // "uptime" is inlined by the caller so its window bind can be
        // interleaved (sqlx 0.9 placeholders come from `push_bind` only).
        "response_time" => {
            "(SELECT c.response_time_ms FROM check_results c \
             WHERE c.monitor_id = m.id AND c.response_time_ms IS NOT NULL \
             ORDER BY c.checked_at DESC LIMIT 1)"
        }
        _ => "m.created_at",
    }
}

/// Page of monitors matching the query, plus the total number of matches.
pub async fn list(
    pool: &sqlx::SqlitePool,
    query: &ListMonitorsQuery,
    window_start: DateTime<Utc>,
) -> Result<(Vec<Monitor>, i64), sqlx::Error> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // --- total count -----------------------------------------------------
    // Placeholders must come from `push_bind`; `?` inside `push()` strings is
    // emitted literally by sqlx 0.9 and breaks the prepared statement.
    let mut count_q = sqlx::QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM monitors m");
    if search.is_some() || status.is_some() {
        count_q.push(" WHERE ");
        let mut sep = "";
        if let Some(pattern) = &search {
            count_q.push("(m.name LIKE ");
            count_q.push_bind(pattern.clone());
            count_q.push(" OR m.url LIKE ");
            count_q.push_bind(pattern.clone());
            count_q.push(")");
            sep = " AND ";
        }
        if let Some(status) = status {
            count_q.push(sep);
            count_q.push("m.current_status = ");
            count_q.push_bind(status.to_string());
        }
    }
    let total: i64 = count_q.build_query_scalar().fetch_one(pool).await?;

    // --- page ------------------------------------------------------------
    let sort = query.sort.as_deref().unwrap_or("created");
    let order = match query.order.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    let mut page_q =
        sqlx::QueryBuilder::<Sqlite>::new(format!("SELECT {SELECT_COLUMNS} FROM monitors m"));
    if search.is_some() || status.is_some() {
        page_q.push(" WHERE ");
        let mut sep = "";
        if let Some(pattern) = &search {
            page_q.push("(m.name LIKE ");
            page_q.push_bind(pattern.clone());
            page_q.push(" OR m.url LIKE ");
            page_q.push_bind(pattern.clone());
            page_q.push(")");
            sep = " AND ";
        }
        if let Some(status) = status {
            page_q.push(sep);
            page_q.push("m.current_status = ");
            page_q.push_bind(status.to_string());
        }
    }

    page_q.push(" ORDER BY ");
    if sort == "uptime" {
        page_q.push(
            "(SELECT AVG(CASE WHEN c.success THEN 1.0 ELSE 0.0 END) \
             FROM check_results c WHERE c.monitor_id = m.id AND c.checked_at >= ",
        );
        page_q.push_bind(window_start);
        page_q.push(")");
    } else {
        page_q.push(sort_expression(sort));
    }
    page_q.push(format!(" {order}, m.created_at DESC LIMIT "));
    page_q.push_bind(page_size);
    page_q.push(" OFFSET ");
    page_q.push_bind(offset);

    let rows = page_q.build_query_as::<Monitor>().fetch_all(pool).await?;
    Ok((rows, total))
}

/// Monitor counts grouped by current status (for the overview census).
pub async fn status_counts(pool: &sqlx::SqlitePool) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as::<_, (String, i64)>(
        "SELECT current_status, COUNT(*) FROM monitors GROUP BY current_status",
    )
    .fetch_all(pool)
    .await
}

const SELECT_ALL_SQL: &str = "SELECT id, name, url, enabled, check_interval_seconds, timeout_ms, \
expected_status_min, expected_status_max, consecutive_failures_threshold, \
consecutive_successes_threshold, retry_max_attempts, alert_webhook_url, alert_email, \
current_status, consecutive_failures, consecutive_successes, created_at, updated_at, \
last_checked_at, next_check_at FROM monitors ORDER BY name, id";

/// All monitors (ascending name), for report aggregation.
pub async fn find_all(pool: &sqlx::SqlitePool) -> Result<Vec<Monitor>, sqlx::Error> {
    sqlx::query_as::<_, Monitor>(SELECT_ALL_SQL)
        .fetch_all(pool)
        .await
}
