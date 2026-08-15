-- Phase 6: listener requests.

CREATE TABLE request_rules (
    station_id   TEXT PRIMARY KEY REFERENCES stations(id) ON DELETE CASCADE,
    enabled      BOOLEAN NOT NULL DEFAULT FALSE,
    max_per_hour BIGINT NOT NULL DEFAULT 5,
    dedupe       BOOLEAN NOT NULL DEFAULT TRUE,
    moderation   BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE requests (
    id           TEXT PRIMARY KEY,
    station_id   TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    media_id     TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    requested_by TEXT,
    status       TEXT NOT NULL DEFAULT 'pending',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE INDEX idx_requests_station_created ON requests(station_id, created_at);
