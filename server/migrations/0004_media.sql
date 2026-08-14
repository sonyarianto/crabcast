-- Phase 3: media library.

CREATE TABLE media_files (
    id                    TEXT PRIMARY KEY,
    sha256                TEXT NOT NULL UNIQUE,
    filename              TEXT NOT NULL,
    mime                  TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes            INTEGER NOT NULL,
    storage_path          TEXT NOT NULL,
    title                 TEXT NOT NULL DEFAULT '',
    artist                TEXT NOT NULL DEFAULT '',
    album                 TEXT NOT NULL DEFAULT '',
    genre                 TEXT NOT NULL DEFAULT '',
    duration_seconds      REAL,
    sample_rate           INTEGER,
    channels              INTEGER,
    bitrate               INTEGER,
    replaygain_track_gain REAL,
    replaygain_album_gain REAL,
    cover_path            TEXT,
    cover_mime            TEXT,
    waveform              TEXT NOT NULL DEFAULT '[]',
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE INDEX idx_media_artist  ON media_files(artist);
CREATE INDEX idx_media_album   ON media_files(album);
CREATE INDEX idx_media_genre   ON media_files(genre);
CREATE INDEX idx_media_created ON media_files(created_at);
