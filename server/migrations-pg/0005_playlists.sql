-- Phase 4: playlists & scheduling.

CREATE TABLE playlists (
    id         TEXT PRIMARY KEY,
    station_id TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL DEFAULT 'standard',
    weight     BIGINT NOT NULL DEFAULT 1,
    shuffle    BOOLEAN NOT NULL DEFAULT FALSE,
    enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_playlists_station ON playlists(station_id);

CREATE TABLE playlist_tracks (
    id          TEXT PRIMARY KEY,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    media_id    TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    position    BIGINT NOT NULL DEFAULT 0,
    fade_in     DOUBLE PRECISION,
    fade_out    DOUBLE PRECISION,
    cue_in      DOUBLE PRECISION,
    cue_out     DOUBLE PRECISION,
    UNIQUE (playlist_id, media_id)
);

CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);

CREATE TABLE playlist_schedules (
    id          TEXT PRIMARY KEY,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    days        TEXT NOT NULL DEFAULT '',
    start_time  TEXT NOT NULL,
    end_time    TEXT NOT NULL
);

CREATE INDEX idx_playlist_schedules_playlist ON playlist_schedules(playlist_id);
