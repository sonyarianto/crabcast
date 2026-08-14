-- Phase 5: streamers (live DJ accounts).

-- Each streamer is a named account on a station with its own Icecast
-- source-protocol password (rendered as an extra valid password on the
-- station's `input.harbor`). `enabled` revokes a streamer without deleting
-- the account; disabled accounts' passwords stop being accepted on the next
-- config re-render.
CREATE TABLE streamers (
    id              TEXT PRIMARY KEY,
    station_id      TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    source_password TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX idx_streamers_station ON streamers(station_id);
