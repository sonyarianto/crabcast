-- Phase 8: analytics & monitoring.

-- One per-minute listener sample per station, polled from the Icecast admin
-- API. `listeners` is the current connection count; `listener_connections`
-- is Icecast's cumulative connection counter for the mount, so unique
-- listeners over a window are approximated by its delta. `reachable`
-- records whether the admin API responded (it drives uptime % and the
-- icecast_unreachable alert).
CREATE TABLE listener_samples (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id           TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    ts                   TEXT NOT NULL,
    listeners            INTEGER NOT NULL DEFAULT 0,
    listener_connections INTEGER NOT NULL DEFAULT 0,
    reachable            INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_listener_samples_station_ts ON listener_samples(station_id, ts);

-- Alert feed: one row per raised alert, closed by `resolved_at`. The dedup
-- key is (station_id, kind) with resolved_at IS NULL — re-firing while an
-- alert is open is a no-op. `station_id` is NULL for global alerts (e.g.
-- disk_low, which is shared across stations). Kinds:
--   icecast_unreachable | dead_air | engine_crash_loop | disk_low
CREATE TABLE alerts (
    id          TEXT PRIMARY KEY,
    station_id  TEXT REFERENCES stations(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,
    severity    TEXT NOT NULL DEFAULT 'warning',  -- warning | error
    title       TEXT NOT NULL,
    detail      TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX idx_alerts_station_created ON alerts(station_id, created_at);
CREATE INDEX idx_alerts_kind_open ON alerts(kind, resolved_at);
