//! Incident persistence: at most one open incident per monitor (enforced by
//! the partial unique index `idx_incidents_one_open`).

use chrono::{DateTime, Utc};
use sqlx::{Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::models::incident::{Incident, IncidentStatus};

const INSERT_SQL: &str = "INSERT INTO incidents (id, monitor_id, started_at, resolved_at, status, \
failure_reason, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

const FIND_OPEN_SQL: &str = "SELECT id, monitor_id, started_at, resolved_at, status, \
failure_reason, created_at, updated_at FROM incidents \
WHERE monitor_id = ? AND resolved_at IS NULL LIMIT 1";

/// The currently open incident for a monitor, if any.
pub async fn find_open<'c, E>(
    executor: E,
    monitor_id: Uuid,
) -> Result<Option<Incident>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    sqlx::query_as::<_, Incident>(FIND_OPEN_SQL)
        .bind(monitor_id)
        .fetch_optional(executor)
        .await
}

/// Open a new incident. Fails with a constraint error if one is already open.
pub async fn open<'c, E>(
    executor: E,
    monitor_id: Uuid,
    started_at: DateTime<Utc>,
    failure_reason: Option<&str>,
) -> Result<Incident, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Sqlite>,
{
    let incident = Incident {
        id: Uuid::new_v4(),
        monitor_id,
        started_at,
        resolved_at: None,
        status: IncidentStatus::Open,
        failure_reason: failure_reason.map(truncate_reason),
        created_at: started_at,
        updated_at: started_at,
    };
    sqlx::query(INSERT_SQL)
        .bind(incident.id)
        .bind(incident.monitor_id)
        .bind(incident.started_at)
        .bind(incident.resolved_at)
        .bind(incident.status)
        .bind(&incident.failure_reason)
        .bind(incident.created_at)
        .bind(incident.updated_at)
        .execute(executor)
        .await?;
    Ok(incident)
}

/// Resolve the open incident for a monitor (if any) and return it.
pub async fn resolve_open(
    conn: &mut SqliteConnection,
    monitor_id: Uuid,
    resolved_at: DateTime<Utc>,
) -> Result<Option<Incident>, sqlx::Error> {
    let Some(open_incident) = find_open(&mut *conn, monitor_id).await? else {
        return Ok(None);
    };
    sqlx::query(
        "UPDATE incidents SET status = 'resolved', resolved_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(resolved_at)
    .bind(resolved_at)
    .bind(open_incident.id)
    .execute(&mut *conn)
    .await?;
    Ok(Some(Incident {
        status: IncidentStatus::Resolved,
        resolved_at: Some(resolved_at),
        updated_at: resolved_at,
        ..open_incident
    }))
}

/// Incident messages are user-visible; keep them bounded.
fn truncate_reason(reason: &str) -> String {
    const MAX: usize = 500;
    let reason = reason.trim();
    if reason.len() <= MAX {
        return reason.to_string();
    }
    let mut end = MAX;
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_string()
}

// ---------------------------------------------------------------------------
// Listing and windowed statistics
// ---------------------------------------------------------------------------

/// Paginated incident list joined with monitor name/url, filtered by
/// status / monitor / started_at window. Returns the page and total count.
pub async fn list_with_monitor(
    pool: &sqlx::SqlitePool,
    status: Option<IncidentStatus>,
    monitor_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<crate::models::incident::IncidentWithMonitor>, i64), sqlx::Error> {
    let build_where = |query: &mut sqlx::QueryBuilder<Sqlite>| {
        query.push(" WHERE ");
        let mut sep = "";
        if let Some(status) = status {
            query.push("i.status = ");
            query.push_bind(status.as_str().to_string());
            sep = " AND ";
        }
        if let Some(id) = monitor_id {
            query.push(sep);
            query.push("i.monitor_id = ");
            query.push_bind(id);
            sep = " AND ";
        }
        if let Some(from) = from {
            query.push(sep);
            query.push("i.started_at >= ");
            query.push_bind(from);
            sep = " AND ";
        }
        if let Some(to) = to {
            query.push(sep);
            query.push("i.started_at < ");
            query.push_bind(to);
        }
    };

    let mut count_q = sqlx::QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM incidents i");
    build_where(&mut count_q);
    let total: i64 = count_q.build_query_scalar().fetch_one(pool).await?;

    let mut page_q = sqlx::QueryBuilder::<Sqlite>::new(
        "SELECT i.id, i.monitor_id, i.started_at, i.resolved_at, i.status, \
         i.failure_reason, i.created_at, i.updated_at, m.name, m.url \
         FROM incidents i JOIN monitors m ON m.id = i.monitor_id",
    );
    build_where(&mut page_q);
    page_q.push(" ORDER BY i.started_at DESC, i.id DESC LIMIT ");
    page_q.push_bind(page_size);
    page_q.push(" OFFSET ");
    page_q.push_bind((page - 1) * page_size);

    let rows = page_q
        .build_query_as::<(
            Uuid,
            Uuid,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
            IncidentStatus,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
            String,
            String,
        )>()
        .fetch_all(pool)
        .await?;

    let now = Utc::now();
    let incidents = rows
        .into_iter()
        .map(
            |(
                id,
                monitor_id,
                started_at,
                resolved_at,
                status,
                failure_reason,
                created_at,
                updated_at,
                monitor_name,
                monitor_url,
            )| {
                let end = resolved_at.unwrap_or(now);
                crate::models::incident::IncidentWithMonitor {
                    incident: crate::models::incident::Incident {
                        id,
                        monitor_id,
                        started_at,
                        resolved_at,
                        status,
                        failure_reason,
                        created_at,
                        updated_at,
                    },
                    monitor_name,
                    monitor_url,
                    duration_seconds: (end - started_at).num_seconds().max(0),
                }
            },
        )
        .collect();
    Ok((incidents, total))
}

/// Incidents that started inside `[start, end)`, with computed durations
/// (open incidents measured at query time). Used for downtime totals.
pub async fn window_incidents(
    pool: &sqlx::SqlitePool,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<(DateTime<Utc>, Option<DateTime<Utc>>)>, sqlx::Error> {
    sqlx::query_as::<_, (DateTime<Utc>, Option<DateTime<Utc>>)>(
        "SELECT started_at, resolved_at FROM incidents \
         WHERE started_at >= ? AND started_at < ? ORDER BY started_at",
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Flat row: incident columns followed by monitor name/url.
type IncidentJoinRow = (
    Uuid,
    Uuid,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    IncidentStatus,
    Option<String>,
    DateTime<Utc>,
    DateTime<Utc>,
    String,
    String,
);

fn join_row_to_with_monitor(
    row: IncidentJoinRow,
    now: DateTime<Utc>,
) -> crate::models::incident::IncidentWithMonitor {
    let (
        id,
        monitor_id,
        started_at,
        resolved_at,
        status,
        failure_reason,
        created_at,
        updated_at,
        monitor_name,
        monitor_url,
    ) = row;
    let end = resolved_at.unwrap_or(now);
    crate::models::incident::IncidentWithMonitor {
        incident: crate::models::incident::Incident {
            id,
            monitor_id,
            started_at,
            resolved_at,
            status,
            failure_reason,
            created_at,
            updated_at,
        },
        monitor_name,
        monitor_url,
        duration_seconds: (end - started_at).num_seconds().max(0),
    }
}

/// Latest incidents across all monitors (overview feed).
pub async fn recent(
    pool: &sqlx::SqlitePool,
    limit: i64,
) -> Result<Vec<crate::models::incident::IncidentWithMonitor>, sqlx::Error> {
    let rows = sqlx::query_as::<_, IncidentJoinRow>(
        "SELECT i.id, i.monitor_id, i.started_at, i.resolved_at, i.status, \
         i.failure_reason, i.created_at, i.updated_at, m.name, m.url \
         FROM incidents i JOIN monitors m ON m.id = i.monitor_id \
         ORDER BY i.started_at DESC, i.id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    Ok(rows
        .into_iter()
        .map(|row| join_row_to_with_monitor(row, now))
        .collect())
}

/// Find one incident by id, joined with its monitor.
pub async fn find_with_monitor(
    pool: &sqlx::SqlitePool,
    id: Uuid,
) -> Result<Option<crate::models::incident::IncidentWithMonitor>, sqlx::Error> {
    let row = sqlx::query_as::<_, IncidentJoinRow>(
        "SELECT i.id, i.monitor_id, i.started_at, i.resolved_at, i.status, \
         i.failure_reason, i.created_at, i.updated_at, m.name, m.url \
         FROM incidents i JOIN monitors m ON m.id = i.monitor_id WHERE i.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| join_row_to_with_monitor(row, Utc::now())))
}
