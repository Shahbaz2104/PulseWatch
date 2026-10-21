//! Idempotent demo fixtures. The public demo boots with believable history
//! (checks, incidents, a resolved outage and one paused monitor) so every
//! dashboard panel has data on first load. Seeding only runs when the
//! monitors table is empty, so restarts and volume re-deploys never duplicate
//! or overwrite user data.

use chrono::{DateTime, Duration, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::state::AppState;

/// Number of rows created by a successful seed. All zero when skipped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SeedOutcome {
    pub monitors: u64,
    pub checks: u64,
    pub incidents: u64,
}

impl SeedOutcome {
    pub fn seeded(&self) -> bool {
        self.monitors > 0
    }
}

/// Seeds demo data only when no monitors exist yet.
pub async fn seed_if_empty(state: &AppState) -> Result<SeedOutcome, sqlx::Error> {
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM monitors")
        .fetch_one(&state.pool)
        .await?;
    if existing > 0 {
        return Ok(SeedOutcome::default());
    }
    seed(&state.pool).await
}

/// Inserts the demo fixture set in a single transaction.
pub async fn seed(pool: &SqlitePool) -> Result<SeedOutcome, sqlx::Error> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;

    let github = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "GitHub",
            url: "https://github.com",
            enabled: true,
            status: "up",
            consecutive_failures: 0,
            consecutive_successes: 12,
            last_checked_at: Some(now - Duration::minutes(1)),
            next_check_at: Some(now),
            alert_email: Some("ops@example.com"),
        },
    )
    .await?;
    let stripe = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "Stripe API",
            url: "https://api.stripe.com",
            enabled: true,
            status: "down",
            consecutive_failures: 2,
            consecutive_successes: 0,
            last_checked_at: Some(now - Duration::minutes(1)),
            next_check_at: Some(now),
            alert_email: Some("ops@example.com"),
        },
    )
    .await?;
    let wikipedia = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "Wikipedia",
            url: "https://www.wikipedia.org",
            enabled: true,
            status: "up",
            consecutive_failures: 0,
            consecutive_successes: 9,
            last_checked_at: Some(now - Duration::minutes(1)),
            next_check_at: Some(now),
            alert_email: None,
        },
    )
    .await?;
    let hacker_news = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "Hacker News",
            url: "https://news.ycombinator.com",
            enabled: true,
            status: "up",
            consecutive_failures: 0,
            consecutive_successes: 5,
            last_checked_at: Some(now - Duration::minutes(1)),
            next_check_at: Some(now),
            alert_email: None,
        },
    )
    .await?;
    let rust_blog = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "Rust Blog",
            url: "https://blog.rust-lang.org",
            enabled: true,
            status: "up",
            consecutive_failures: 0,
            consecutive_successes: 4,
            last_checked_at: Some(now - Duration::minutes(2)),
            next_check_at: Some(now),
            alert_email: None,
        },
    )
    .await?;
    let maintenance = insert_monitor(
        &mut tx,
        &MonitorSpec {
            name: "Maintenance Window",
            url: "https://status.example.com",
            enabled: false,
            status: "paused",
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_checked_at: Some(now - Duration::hours(5)),
            next_check_at: None,
            alert_email: None,
        },
    )
    .await?;

    let mut checks = 0_u64;
    for hours_ago in (0..24).rev() {
        let checked_at = now - Duration::hours(hours_ago);
        let response_time = 90 + (hours_ago * 37) % 260;
        insert_check(
            &mut tx,
            github,
            checked_at,
            true,
            Some(200),
            response_time,
            None,
        )
        .await?;
        insert_check(
            &mut tx,
            hacker_news,
            checked_at,
            true,
            Some(200),
            response_time + 45,
            None,
        )
        .await?;
        insert_check(
            &mut tx,
            rust_blog,
            checked_at,
            true,
            Some(200),
            response_time + 70,
            None,
        )
        .await?;

        // Stripe: healthy until two hours ago, then fails as the open incident.
        let stripe_failing = hours_ago < 2;
        insert_check(
            &mut tx,
            stripe,
            checked_at,
            !stripe_failing,
            Some(if stripe_failing { 503 } else { 200 }),
            response_time + 20,
            stripe_failing.then_some("http_status_mismatch"),
        )
        .await?;

        // Wikipedia: a two-hour outage yesterday, already resolved.
        let wikipedia_failing = (18..=19).contains(&hours_ago);
        insert_check(
            &mut tx,
            wikipedia,
            checked_at,
            !wikipedia_failing,
            Some(if wikipedia_failing { 504 } else { 200 }),
            response_time + 60,
            wikipedia_failing.then_some("timeout"),
        )
        .await?;
    }
    checks += 24 * 5;

    // A short check history before the maintenance pause began.
    for hours_ago in (5..=8).rev() {
        let checked_at = now - Duration::hours(hours_ago);
        insert_check(
            &mut tx,
            maintenance,
            checked_at,
            true,
            Some(200),
            130 + hours_ago * 9,
            None,
        )
        .await?;
        checks += 1;
    }

    let mut incidents = 0_u64;
    insert_incident(
        &mut tx,
        stripe,
        now - Duration::hours(2),
        None,
        Some("HTTP 503 across 2 attempts"),
        now - Duration::hours(2),
    )
    .await?;
    incidents += 1;
    insert_incident(
        &mut tx,
        wikipedia,
        now - Duration::hours(19),
        Some(now - Duration::hours(18)),
        Some("request timed out"),
        now - Duration::hours(19),
    )
    .await?;
    incidents += 1;

    insert_pause(&mut tx, maintenance, now - Duration::hours(3)).await?;

    tx.commit().await?;

    Ok(SeedOutcome {
        monitors: 6,
        checks,
        incidents,
    })
}

struct MonitorSpec<'a> {
    name: &'a str,
    url: &'a str,
    enabled: bool,
    status: &'a str,
    consecutive_failures: i64,
    consecutive_successes: i64,
    last_checked_at: Option<DateTime<Utc>>,
    next_check_at: Option<DateTime<Utc>>,
    alert_email: Option<&'a str>,
}

async fn insert_monitor(
    conn: &mut SqliteConnection,
    spec: &MonitorSpec<'_>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO monitors (
            id, name, url, enabled, check_interval_seconds, timeout_ms,
            expected_status_min, expected_status_max,
            consecutive_failures_threshold, consecutive_successes_threshold,
            retry_max_attempts, current_status, consecutive_failures,
            consecutive_successes, created_at, updated_at, last_checked_at,
            next_check_at, alert_email
        ) VALUES (?, ?, ?, ?, 60, 5000, 200, 399, 2, 2, 2, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(spec.name)
    .bind(spec.url)
    .bind(spec.enabled)
    .bind(spec.status)
    .bind(spec.consecutive_failures)
    .bind(spec.consecutive_successes)
    .bind(now - Duration::days(7))
    .bind(now)
    .bind(spec.last_checked_at)
    .bind(spec.next_check_at)
    .bind(spec.alert_email)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}

async fn insert_check(
    conn: &mut SqliteConnection,
    monitor_id: Uuid,
    checked_at: DateTime<Utc>,
    success: bool,
    http_status_code: Option<i64>,
    response_time_ms: i64,
    error_type: Option<&str>,
) -> Result<(), sqlx::Error> {
    let completed_at = checked_at + Duration::milliseconds(response_time_ms);
    sqlx::query(
        "INSERT INTO check_results (
            id, monitor_id, checked_at, completed_at, http_status_code,
            response_time_ms, success, error_type, error_message,
            attempt_count, expected_status_match
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_id)
    .bind(checked_at)
    .bind(completed_at)
    .bind(http_status_code)
    .bind(response_time_ms)
    .bind(success)
    .bind(error_type)
    .bind(error_type.map(|_| "demo fixture failure".to_string()))
    .bind(if success { 1_i64 } else { 2 })
    .bind(success)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_incident(
    conn: &mut SqliteConnection,
    monitor_id: Uuid,
    started_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
    failure_reason: Option<&str>,
    updated_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO incidents (
            id, monitor_id, started_at, resolved_at, status, failure_reason,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_id)
    .bind(started_at)
    .bind(resolved_at)
    .bind(if resolved_at.is_some() {
        "resolved"
    } else {
        "open"
    })
    .bind(failure_reason)
    .bind(started_at)
    .bind(updated_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_pause(
    conn: &mut SqliteConnection,
    monitor_id: Uuid,
    paused_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO monitor_pauses (id, monitor_id, paused_at, resumed_at) VALUES (?, ?, ?, NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(monitor_id)
    .bind(paused_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
