//! Shared application state available to every route handler.

use std::collections::HashSet;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::config::Config;

/// Tracks checks currently executing so scheduled and manual checks for the
/// same monitor never overlap. The inner lock is a Tokio mutex and is never
/// held across other `.await` points — the guard is scoped to insert/remove.
#[derive(Debug, Default)]
pub struct InFlightSet(Mutex<HashSet<Uuid>>);

impl InFlightSet {
    /// Returns `true` when this monitor id was not already in flight.
    pub async fn try_begin(&self, monitor_id: Uuid) -> bool {
        let mut guard = self.0.lock().await;
        guard.insert(monitor_id)
    }

    pub async fn end(&self, monitor_id: Uuid) {
        let mut guard = self.0.lock().await;
        guard.remove(&monitor_id);
    }

    pub async fn contains(&self, monitor_id: Uuid) -> bool {
        let guard = self.0.lock().await;
        guard.contains(&monitor_id)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub http_client: Arc<reqwest::Client>,
    /// Bounds concurrent outbound checks across scheduled + manual triggers.
    pub semaphore: Arc<Semaphore>,
    pub in_flight: Arc<InFlightSet>,
    /// Cancelled on SIGINT/SIGTERM for graceful shutdown of scheduler/tasks.
    pub shutdown: CancellationToken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_flight_set_prevents_duplicates() {
        let set = InFlightSet::default();
        let id = Uuid::new_v4();
        assert!(set.try_begin(id).await);
        assert!(!set.try_begin(id).await);
        assert!(set.contains(id).await);
        set.end(id).await;
        assert!(set.try_begin(id).await);
    }
}
