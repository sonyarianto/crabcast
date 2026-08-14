-- Phase 4: playlists & scheduling.

-- Playlist types (kind): standard (shuffle/sequential), looping,
-- scheduled (dayparted via switch), once_per_hour (AzuraCast parity).
-- `weight` scales how often the playlist is picked relative to siblings
-- (mapped to crabsoup `rotate` weights); `shuffle` toggles playlist-level
-- shuffle.
CREATE TABLE playlists (
    id          TEXT PRIMARY KEY,
    station_id  TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'standard',
    weight      INTEGER NOT NULL DEFAULT 1,
    shuffle     INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_playlists_station ON playlists(station_id);

-- Ordering + per-track fade/cue overrides, mapped to crabsoup `annotate:`
-- prefixes on each file entry. position is 0-based within the playlist.
CREATE TABLE playlist_tracks (
    id          TEXT PRIMARY KEY,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    media_id    TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    position    INTEGER NOT NULL DEFAULT 0,
    fade_in     REAL,
    fade_out    REAL,
    cue_in      REAL,
    cue_out     REAL,
    UNIQUE (playlist_id, media_id)
);

CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);

-- Daypart rules for `scheduled` playlists: weekdays as comma-separated
-- names ("mon,tue"), times as "HH:MM". Overnight windows wrap.
CREATE TABLE playlist_schedules (
    id          TEXT PRIMARY KEY,
    playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    days        TEXT NOT NULL DEFAULT '',
    start_time  TEXT NOT NULL,
    end_time    TEXT NOT NULL
);

CREATE INDEX idx_playlist_schedules_playlist ON playlist_schedules(playlist_id);
