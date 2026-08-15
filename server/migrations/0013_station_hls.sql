-- HLS streaming (Phase 11): the engine taps the mix to MPEG-TS segments
-- in an admin-configured directory when hls_enabled is set. segment_seconds
-- and retention mirror the engine's HlsOutputConfig defaults.
ALTER TABLE stations ADD COLUMN hls_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE stations ADD COLUMN hls_dir TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN hls_segment_seconds REAL NOT NULL DEFAULT 5.0;
ALTER TABLE stations ADD COLUMN hls_retention INTEGER NOT NULL DEFAULT 12;
