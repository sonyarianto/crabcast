-- Phase 2: auth, users, roles, audit log.

CREATE TABLE users (
    id             TEXT PRIMARY KEY,
    username       TEXT NOT NULL UNIQUE,
    password_hash  TEXT NOT NULL,
    display_name   TEXT NOT NULL DEFAULT '',
    is_super_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE TABLE roles (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT ''
);

INSERT INTO roles (id, name, description) VALUES
    ('role-station-manager', 'station_manager', 'Manage stations, playlists and media'),
    ('role-dj',              'dj',              'Control a station live: skip, queue, jingles'),
    ('role-media-editor',    'media_editor',    'Edit media files and metadata');

-- A NULL station_id means the grant applies to every station (global).
CREATE TABLE user_roles (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id    TEXT NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    station_id TEXT REFERENCES stations(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id, station_id)
);

CREATE TABLE audit_log (
    id         BIGSERIAL PRIMARY KEY,
    user_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
    action     TEXT NOT NULL,
    target     TEXT NOT NULL DEFAULT '',
    detail     TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE INDEX idx_user_roles_user ON user_roles(user_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);

-- Session store table. The tower-sessions PgStore defaults to schema
-- `tower_sessions`, table `session` (id TEXT, data BYTEA, expiry_date
-- TIMESTAMPTZ); create both so its queries resolve.
CREATE SCHEMA IF NOT EXISTS tower_sessions;
CREATE TABLE IF NOT EXISTS tower_sessions.session (
    id           TEXT PRIMARY KEY NOT NULL,
    data         BYTEA NOT NULL,
    expiry_date  TIMESTAMPTZ NOT NULL
);
