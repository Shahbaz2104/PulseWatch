-- Paused intervals preserve when checks were intentionally suspended.
-- At most one interval per monitor may be open at a time.

CREATE TABLE monitor_pauses (
    id         TEXT PRIMARY KEY NOT NULL,
    monitor_id TEXT NOT NULL
        REFERENCES monitors (id) ON DELETE CASCADE,
    paused_at  TEXT NOT NULL,
    resumed_at TEXT
);

CREATE INDEX idx_monitor_pauses_monitor_time
    ON monitor_pauses (monitor_id, paused_at DESC);

-- One open pause per monitor, even if pause is requested concurrently.
CREATE UNIQUE INDEX idx_monitor_pauses_one_open ON monitor_pauses (monitor_id)
    WHERE resumed_at IS NULL;

-- Supports the retention worker's completed-delivery sweep.
CREATE INDEX idx_alert_deliveries_created_at ON alert_deliveries (created_at);
