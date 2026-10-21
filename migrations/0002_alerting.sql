-- Alert destinations on monitors and the reliable delivery outbox.
-- Delivery rows are written in the same transaction as the incident
-- transition, so an incident can never exist without its alerts being queued.

ALTER TABLE monitors ADD COLUMN alert_webhook_url TEXT;
ALTER TABLE monitors ADD COLUMN alert_email TEXT;

CREATE TABLE alert_deliveries (
    id             TEXT PRIMARY KEY NOT NULL,
    monitor_id     TEXT NOT NULL
        REFERENCES monitors (id) ON DELETE CASCADE,
    incident_id    TEXT NOT NULL
        REFERENCES incidents (id) ON DELETE CASCADE,
    event          TEXT NOT NULL
        CHECK (event IN ('opened', 'resolved')),
    channel        TEXT NOT NULL
        CHECK (channel IN ('webhook', 'email')),
    destination    TEXT NOT NULL,
    payload        TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'delivered', 'failed')),
    attempts       INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NOT NULL,
    last_error     TEXT,
    created_at     TEXT NOT NULL,
    delivered_at   TEXT
);

-- One delivery per incident transition and channel, even if a transaction is
-- retried or an alert worker crashes mid-delivery.
CREATE UNIQUE INDEX idx_alert_deliveries_incident_event_channel
    ON alert_deliveries (incident_id, event, channel);
CREATE INDEX idx_alert_deliveries_due
    ON alert_deliveries (status, next_attempt_at);
CREATE INDEX idx_alert_deliveries_monitor
    ON alert_deliveries (monitor_id, created_at DESC);
