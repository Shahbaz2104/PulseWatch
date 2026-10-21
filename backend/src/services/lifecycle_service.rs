//! Transactional monitor lifecycle operations.
//!
//! Pausing records a pause interval and resolves any open incident at the
//! same instant (silently — no recovery alert). Resuming closes that interval.
//! Both operations are atomic so a monitor can never be paused without its
//! incident state being settled.

use chrono::Utc;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::monitor::Monitor;
use crate::repositories::{incident_repository, monitor_repository, pause_repository};
use crate::state::AppState;

/// Effect of pausing a monitor.
#[derive(Debug)]
pub struct PauseOutcome {
    pub monitor: Monitor,
    /// Open incident resolved by the pause, if there was one.
    pub resolved_incident_id: Option<Uuid>,
}

/// Effect of resuming a monitor.
#[derive(Debug)]
pub struct ResumeOutcome {
    pub monitor: Monitor,
    /// Whether an open pause interval was closed.
    pub closed_interval: bool,
}

/// Pause a monitor, record the interval, and resolve any open incident at
/// `now`. Idempotent: pausing an already-paused monitor changes nothing.
pub async fn pause_monitor(state: &AppState, id: Uuid) -> Result<PauseOutcome, AppError> {
    let mut tx = state.pool.begin().await?;
    let monitor = monitor_repository::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))?;

    if !monitor.enabled {
        return Ok(PauseOutcome {
            monitor,
            resolved_incident_id: None,
        });
    }

    let now = Utc::now();
    if !monitor_repository::pause(&mut *tx, id, now).await? {
        return Err(AppError::Internal(
            "monitor pause update did not apply".to_string(),
        ));
    }
    pause_repository::open(&mut *tx, id, now).await?;
    let resolved = incident_repository::resolve_open(&mut tx, id, now).await?;
    let monitor = monitor_repository::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::Internal("monitor disappeared during pause".to_string()))?;
    tx.commit().await?;

    Ok(PauseOutcome {
        monitor,
        resolved_incident_id: resolved.map(|incident| incident.id),
    })
}

/// Resume a paused monitor and close its open pause interval. Idempotent:
/// resuming an enabled monitor changes nothing.
pub async fn resume_monitor(state: &AppState, id: Uuid) -> Result<ResumeOutcome, AppError> {
    let mut tx = state.pool.begin().await?;
    let monitor = monitor_repository::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::NotFound("monitor not found".to_string()))?;

    if monitor.enabled {
        return Ok(ResumeOutcome {
            monitor,
            closed_interval: false,
        });
    }

    let now = Utc::now();
    let closed_interval = pause_repository::close_open(&mut *tx, id, now).await?;
    if !monitor_repository::resume(&mut *tx, id, now, now).await? {
        return Err(AppError::Internal(
            "monitor resume update did not apply".to_string(),
        ));
    }
    let monitor = monitor_repository::find_by_id(&mut *tx, id)
        .await?
        .ok_or_else(|| AppError::Internal("monitor disappeared during resume".to_string()))?;
    tx.commit().await?;

    Ok(ResumeOutcome {
        monitor,
        closed_interval,
    })
}
