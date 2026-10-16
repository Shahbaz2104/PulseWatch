-- PulseWatch initial schema
-- Timestamps are stored by SQLx as UTC text in ascending-lexicographic order.

CREATE TABLE monitors (
    id                              TEXT PRIMARY KEY NOT NULL,
    name                            TEXT NOT NULL,
    url                             TEXT NOT NULL,
    enabled                         INTEGER NOT NULL DEFAULT 1,
    check_interval_seconds          INTEGER NOT NULL DEFAULT 60,
    timeout_ms                      INTEGER NOT NULL DEFAULT 5000,
    expected_status_min             INTEGER NOT NULL DEFAULT 200,
    expected_status_max             INTEGER NOT NULL DEFAULT 399,
    consecutive_failures_threshold  INTEGER NOT NULL DEFAULT 1,
    consecutive_successes_threshold INTEGER NOT NULL DEFAULT 1,
    retry_max_attempts              INTEGER NOT NULL DEFAULT 2,
    current_status                  TEXT NOT NULL DEFAULT 'unknown'
        CHECK (current_status IN ('unknown', 'up', 'down', 'paused')),
    consecutive_failures            INTEGER NOT NULL DEFAULT 0,
    consecutive_successes           INTEGER NOT NULL DEFAULT 0,
    created_at                      TEXT NOT NULL,
    updated_at                      TEXT NOT NULL,
    last_checked_at                 TEXT,
    next_check_at                   TEXT
);

CREATE INDEX idx_monitors_due ON monitors (enabled, next_check_at);
CREATE INDEX idx_monitors_status ON monitors (current_status);

CREATE TABLE check_results (
    id                    TEXT PRIMARY KEY NOT NULL,
    monitor_id            TEXT NOT NULL
        REFERENCES monitors (id) ON DELETE CASCADE,
    checked_at            TEXT NOT NULL,
    completed_at          TEXT,
    http_status_code      INTEGER,
    response_time_ms      INTEGER,
    success               INTEGER NOT NULL,
    error_type            TEXT,
    error_message         TEXT,
    attempt_count         INTEGER NOT NULL DEFAULT 1,
    expected_status_match INTEGER NOT NULL
);

CREATE INDEX idx_checks_monitor_time ON check_results (monitor_id, checked_at DESC);
CREATE INDEX idx_checks_time ON check_results (checked_at DESC);

CREATE TABLE incidents (
    id             TEXT PRIMARY KEY NOT NULL,
    monitor_id     TEXT NOT NULL
        REFERENCES monitors (id) ON DELETE CASCADE,
    started_at     TEXT NOT NULL,
    resolved_at    TEXT,
    status         TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'resolved')),
    failure_reason TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX idx_incidents_monitor_start ON incidents (monitor_id, started_at DESC);
CREATE INDEX idx_incidents_start ON incidents (started_at DESC);
-- At most one open incident per monitor.
CREATE UNIQUE INDEX idx_incidents_one_open ON incidents (monitor_id)
    WHERE resolved_at IS NULL;

CREATE TABLE settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
