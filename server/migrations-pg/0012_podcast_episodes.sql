-- Phase 11 stretch: podcast episodes (AzuraCast parity).
CREATE TABLE podcast_episodes (
    id          TEXT PRIMARY KEY,
    station_id  TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    media_id    TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
);

CREATE INDEX idx_podcast_episodes_station_created
    ON podcast_episodes(station_id, created_at);
