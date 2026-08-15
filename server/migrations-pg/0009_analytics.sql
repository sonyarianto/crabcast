-- Phase 8: analytics & monitoring.

CREATE TABLE listener_samples (
    id                   BIGSERIAL PRIMARY KEY,
    station_id           TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    ts                   TEXT NOT NULL,
    listeners            BIGINT NOT NULL DEFAULT 0,
    listener_connections BIGINT NOT NULL DEFAULT 0,
    reachable            BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX idx_listener_samples_station_ts ON listener_samples(station_id, ts);

CREATE TABLE alerts (
    id          TEXT PRIMARY KEY,
    station_id  TEXT REFERENCES stations(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    severity    TEXT NOT NULL DEFAULT 'warning',
    title       TEXT NOT NULL,
    detail      TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX idx_alerts_station_created ON alerts(station_id, created_at);
CREATE INDEX idx_alerts_kind_open ON alerts(kind, resolved_at);
