-- Phase 1: station control-plane config + song history.
-- Stations gain the engine-facing knobs the lua/ generator needs; song
-- history rows are pushed from the engine via the track webhook.

ALTER TABLE stations ADD COLUMN sample_rate INTEGER NOT NULL DEFAULT 44100;
ALTER TABLE stations ADD COLUMN channels INTEGER NOT NULL DEFAULT 2;
ALTER TABLE stations ADD COLUMN frames_per_buffer INTEGER NOT NULL DEFAULT 4096;
ALTER TABLE stations ADD COLUMN crossfade_seconds REAL NOT NULL DEFAULT 3.0;
ALTER TABLE stations ADD COLUMN fade_curve REAL NOT NULL DEFAULT 1.0;
ALTER TABLE stations ADD COLUMN duck_seconds REAL NOT NULL DEFAULT 1.5;

ALTER TABLE stations ADD COLUMN playlist_dir TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN jingles_dir TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN harbor_port INTEGER NOT NULL DEFAULT 8005;
ALTER TABLE stations ADD COLUMN harbor_mount TEXT NOT NULL DEFAULT '/live';
ALTER TABLE stations ADD COLUMN harbor_password TEXT NOT NULL DEFAULT 'dj';

ALTER TABLE stations ADD COLUMN control_port INTEGER NOT NULL DEFAULT 1234;
ALTER TABLE stations ADD COLUMN control_http_port INTEGER NOT NULL DEFAULT 9234;

ALTER TABLE stations ADD COLUMN icecast_host TEXT NOT NULL DEFAULT 'localhost';
ALTER TABLE stations ADD COLUMN icecast_port INTEGER NOT NULL DEFAULT 8000;
ALTER TABLE stations ADD COLUMN icecast_mount TEXT NOT NULL DEFAULT '/radio';
ALTER TABLE stations ADD COLUMN icecast_format TEXT NOT NULL DEFAULT 'mp3';
ALTER TABLE stations ADD COLUMN icecast_bitrate INTEGER NOT NULL DEFAULT 128000;
ALTER TABLE stations ADD COLUMN icecast_source_user TEXT NOT NULL DEFAULT 'source';
ALTER TABLE stations ADD COLUMN icecast_source_password TEXT NOT NULL DEFAULT 'hackme';

ALTER TABLE stations ADD COLUMN updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TABLE IF NOT EXISTS song_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id  TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ended_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_song_history_station_started
    ON song_history (station_id, started_at DESC);