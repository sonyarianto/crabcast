-- Phase 1: station control-plane config + song history.

ALTER TABLE stations ADD COLUMN sample_rate BIGINT NOT NULL DEFAULT 44100;
ALTER TABLE stations ADD COLUMN channels BIGINT NOT NULL DEFAULT 2;
ALTER TABLE stations ADD COLUMN frames_per_buffer BIGINT NOT NULL DEFAULT 4096;
ALTER TABLE stations ADD COLUMN crossfade_seconds DOUBLE PRECISION NOT NULL DEFAULT 3.0;
ALTER TABLE stations ADD COLUMN fade_curve DOUBLE PRECISION NOT NULL DEFAULT 1.0;
ALTER TABLE stations ADD COLUMN duck_seconds DOUBLE PRECISION NOT NULL DEFAULT 1.5;

ALTER TABLE stations ADD COLUMN playlist_dir TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN jingles_dir TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN harbor_port BIGINT NOT NULL DEFAULT 8005;
ALTER TABLE stations ADD COLUMN harbor_mount TEXT NOT NULL DEFAULT '/live';
ALTER TABLE stations ADD COLUMN harbor_password TEXT NOT NULL DEFAULT 'dj';

ALTER TABLE stations ADD COLUMN control_port BIGINT NOT NULL DEFAULT 1234;
ALTER TABLE stations ADD COLUMN control_http_port BIGINT NOT NULL DEFAULT 9234;

ALTER TABLE stations ADD COLUMN icecast_host TEXT NOT NULL DEFAULT 'localhost';
ALTER TABLE stations ADD COLUMN icecast_port BIGINT NOT NULL DEFAULT 8000;
ALTER TABLE stations ADD COLUMN icecast_mount TEXT NOT NULL DEFAULT '/radio';
ALTER TABLE stations ADD COLUMN icecast_format TEXT NOT NULL DEFAULT 'mp3';
ALTER TABLE stations ADD COLUMN icecast_bitrate BIGINT NOT NULL DEFAULT 128000;
ALTER TABLE stations ADD COLUMN icecast_source_user TEXT NOT NULL DEFAULT 'source';
ALTER TABLE stations ADD COLUMN icecast_source_password TEXT NOT NULL DEFAULT 'hackme';

ALTER TABLE stations ADD COLUMN updated_at TEXT NOT NULL DEFAULT (to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'));

CREATE TABLE IF NOT EXISTS song_history (
    id          BIGSERIAL PRIMARY KEY,
    station_id  TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    started_at  TEXT NOT NULL DEFAULT (to_char(now(), 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')),
    ended_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_song_history_station_started
    ON song_history (station_id, started_at DESC);
