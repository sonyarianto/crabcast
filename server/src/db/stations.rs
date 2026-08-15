//! Station model + repository (Phase 1 control plane).

use serde::{Deserialize, Serialize};
use sqlx::AnyPool;
use sqlx::FromRow;

use crate::api::error::ApiError;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,

    pub sample_rate: i64,
    pub channels: i64,
    pub frames_per_buffer: i64,
    pub crossfade_seconds: f64,
    pub fade_curve: f64,
    pub duck_seconds: f64,

    pub playlist_dir: String,
    pub jingles_dir: String,
    pub harbor_port: i64,
    pub harbor_mount: String,
    pub harbor_password: String,

    pub control_port: i64,
    pub control_http_port: i64,

    pub icecast_host: String,
    pub icecast_port: i64,
    pub icecast_mount: String,
    pub icecast_format: String,
    pub icecast_bitrate: i64,
    pub icecast_source_user: String,
    pub icecast_source_password: String,

    pub hls_enabled: crate::db::DbBool,
    pub hls_dir: String,
    pub hls_segment_seconds: f64,
    pub hls_retention: i64,

    pub website: String,
    pub facebook: String,
    pub twitter: String,
    pub instagram: String,
}

/// Fields a client may set when creating/updating a station.
#[derive(Debug, Clone, Deserialize)]
pub struct StationInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_sr")]
    pub sample_rate: i64,
    #[serde(default = "default_channels")]
    pub channels: i64,
    #[serde(default = "default_fpb")]
    pub frames_per_buffer: i64,
    #[serde(default = "default_crossfade")]
    pub crossfade_seconds: f64,
    #[serde(default = "default_fade_curve")]
    pub fade_curve: f64,
    #[serde(default = "default_duck")]
    pub duck_seconds: f64,
    pub playlist_dir: String,
    #[serde(default)]
    pub jingles_dir: String,
    #[serde(default = "default_harbor_port")]
    pub harbor_port: i64,
    #[serde(default = "default_harbor_mount")]
    pub harbor_mount: String,
    #[serde(default = "default_harbor_password")]
    pub harbor_password: String,
    #[serde(default = "default_control_port")]
    pub control_port: i64,
    #[serde(default = "default_control_http_port")]
    pub control_http_port: i64,
    #[serde(default = "default_icecast_host")]
    pub icecast_host: String,
    #[serde(default = "default_icecast_port")]
    pub icecast_port: i64,
    #[serde(default = "default_icecast_mount")]
    pub icecast_mount: String,
    #[serde(default = "default_icecast_format")]
    pub icecast_format: String,
    #[serde(default = "default_icecast_bitrate")]
    pub icecast_bitrate: i64,
    #[serde(default = "default_icecast_source_user")]
    pub icecast_source_user: String,
    #[serde(default = "default_icecast_source_password")]
    pub icecast_source_password: String,

    #[serde(default)]
    pub hls_enabled: crate::db::DbBool,
    #[serde(default)]
    pub hls_dir: String,
    #[serde(default = "default_hls_segment_seconds")]
    pub hls_segment_seconds: f64,
    #[serde(default = "default_hls_retention")]
    pub hls_retention: i64,

    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub facebook: String,
    #[serde(default)]
    pub twitter: String,
    #[serde(default)]
    pub instagram: String,
}

// Defaults mirror the migration's column defaults.
fn default_sr() -> i64 {
    44100
}
fn default_channels() -> i64 {
    2
}
fn default_fpb() -> i64 {
    4096
}
fn default_crossfade() -> f64 {
    3.0
}
fn default_fade_curve() -> f64 {
    1.0
}
fn default_duck() -> f64 {
    1.5
}
fn default_harbor_port() -> i64 {
    8005
}
fn default_harbor_mount() -> String {
    "/live".into()
}
fn default_harbor_password() -> String {
    "dj".into()
}
fn default_control_port() -> i64 {
    1234
}
fn default_control_http_port() -> i64 {
    9234
}
fn default_icecast_host() -> String {
    "localhost".into()
}
fn default_icecast_port() -> i64 {
    8000
}
fn default_icecast_mount() -> String {
    "/radio".into()
}
fn default_icecast_format() -> String {
    "mp3".into()
}
fn default_icecast_bitrate() -> i64 {
    128000
}
fn default_icecast_source_user() -> String {
    "source".into()
}
fn default_icecast_source_password() -> String {
    "hackme".into()
}
fn default_hls_segment_seconds() -> f64 {
    5.0
}
fn default_hls_retention() -> i64 {
    12
}

const COLUMNS: &str = "id, name, description, created_at, sample_rate, channels, \
frames_per_buffer, crossfade_seconds, fade_curve, duck_seconds, playlist_dir, \
jingles_dir, harbor_port, harbor_mount, harbor_password, control_port, \
control_http_port, icecast_host, icecast_port, icecast_mount, icecast_format, \
icecast_bitrate, icecast_source_user, icecast_source_password, hls_enabled, \
hls_dir, hls_segment_seconds, hls_retention, website, facebook, twitter, \
instagram";

pub async fn list(pool: &AnyPool) -> Result<Vec<Station>, ApiError> {
    Ok(
        sqlx::query_as::<_, Station>(&format!("SELECT {COLUMNS} FROM stations ORDER BY name"))
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get(pool: &AnyPool, id: &str) -> Result<Station, ApiError> {
    sqlx::query_as::<_, Station>(&format!("SELECT {COLUMNS} FROM stations WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("station", id))
}

pub async fn create(pool: &AnyPool, input: &StationInput) -> Result<Station, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stations (id, name, description, sample_rate, channels, \
frames_per_buffer, crossfade_seconds, fade_curve, duck_seconds, playlist_dir, \
jingles_dir, harbor_port, harbor_mount, harbor_password, control_port, \
control_http_port, icecast_host, icecast_port, icecast_mount, icecast_format, \
icecast_bitrate, icecast_source_user, icecast_source_password, hls_enabled, \
hls_dir, hls_segment_seconds, hls_retention, website, facebook, twitter, \
instagram) \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.sample_rate)
    .bind(input.channels)
    .bind(input.frames_per_buffer)
    .bind(input.crossfade_seconds)
    .bind(input.fade_curve)
    .bind(input.duck_seconds)
    .bind(&input.playlist_dir)
    .bind(&input.jingles_dir)
    .bind(input.harbor_port)
    .bind(&input.harbor_mount)
    .bind(&input.harbor_password)
    .bind(input.control_port)
    .bind(input.control_http_port)
    .bind(&input.icecast_host)
    .bind(input.icecast_port)
    .bind(&input.icecast_mount)
    .bind(&input.icecast_format)
    .bind(input.icecast_bitrate)
    .bind(&input.icecast_source_user)
    .bind(&input.icecast_source_password)
    .bind(input.hls_enabled)
    .bind(&input.hls_dir)
    .bind(input.hls_segment_seconds)
    .bind(input.hls_retention)
    .bind(&input.website)
    .bind(&input.facebook)
    .bind(&input.twitter)
    .bind(&input.instagram)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn update(pool: &AnyPool, id: &str, input: &StationInput) -> Result<Station, ApiError> {
    let affected = sqlx::query(&format!(
        "UPDATE stations SET name = $1, description = $2, sample_rate = $3, \
channels = $4, frames_per_buffer = $5, crossfade_seconds = $6, fade_curve = $7, \
duck_seconds = $8, playlist_dir = $9, jingles_dir = $10, harbor_port = $11, \
harbor_mount = $12, harbor_password = $13, control_port = $14, control_http_port = $15, \
icecast_host = $16, icecast_port = $17, icecast_mount = $18, icecast_format = $19, \
icecast_bitrate = $20, icecast_source_user = $21, icecast_source_password = $22, \
hls_enabled = $23, hls_dir = $24, hls_segment_seconds = $25, hls_retention = $26, \
website = $27, facebook = $28, twitter = $29, instagram = $30, \
updated_at = {} WHERE id = $31",
        crate::db::now_sql(),
    ))
    .bind(&input.name)
    .bind(&input.description)
    .bind(input.sample_rate)
    .bind(input.channels)
    .bind(input.frames_per_buffer)
    .bind(input.crossfade_seconds)
    .bind(input.fade_curve)
    .bind(input.duck_seconds)
    .bind(&input.playlist_dir)
    .bind(&input.jingles_dir)
    .bind(input.harbor_port)
    .bind(&input.harbor_mount)
    .bind(&input.harbor_password)
    .bind(input.control_port)
    .bind(input.control_http_port)
    .bind(&input.icecast_host)
    .bind(input.icecast_port)
    .bind(&input.icecast_mount)
    .bind(&input.icecast_format)
    .bind(input.icecast_bitrate)
    .bind(&input.icecast_source_user)
    .bind(&input.icecast_source_password)
    .bind(input.hls_enabled)
    .bind(&input.hls_dir)
    .bind(input.hls_segment_seconds)
    .bind(input.hls_retention)
    .bind(&input.website)
    .bind(&input.facebook)
    .bind(&input.twitter)
    .bind(&input.instagram)
    .bind(id)
    .execute(pool)
    .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("station", id));
    }
    get(pool, id).await
}

pub async fn delete(pool: &AnyPool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("DELETE FROM stations WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if affected.rows_affected() == 0 {
        return Err(ApiError::not_found("station", id));
    }
    Ok(())
}
