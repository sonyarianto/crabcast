-- Phase 6: listener requests.

-- Per-station request rules. `max_per_hour` caps how many requests are
-- accepted in any rolling hour; `dedupe` rejects a track that is already
-- queued or was requested recently; `moderation` holds new requests in a
-- pending state until a station manager approves them (otherwise they go
-- straight to the engine queue).
CREATE TABLE request_rules (
    station_id   TEXT PRIMARY KEY REFERENCES stations(id) ON DELETE CASCADE,
    enabled      INTEGER NOT NULL DEFAULT 0,
    max_per_hour INTEGER NOT NULL DEFAULT 5,
    dedupe       INTEGER NOT NULL DEFAULT 1,
    moderation   INTEGER NOT NULL DEFAULT 0
);

-- One row per listener request. Status: pending (awaiting approval),
-- queued (accepted / approved, pushed to the engine), rejected.
CREATE TABLE requests (
    id            TEXT PRIMARY KEY,
    station_id    TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    media_id      TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    requested_by  TEXT,
    status        TEXT NOT NULL DEFAULT 'pending',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX idx_requests_station_created ON requests(station_id, created_at);
