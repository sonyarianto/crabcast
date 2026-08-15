//! Station routes: CRUD, live status, control commands, SSE stream, and
//! the engine track webhook receiver.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tokio_stream::StreamExt;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::api::sse::{StationEvent, StatusEvent, TrackEvent, sse_frame};
use crate::auth::{Csrf, CurrentUser};
use crate::control::ControlClient;
use crate::db::song_history;
use crate::db::stations::{self, Station, StationInput};
use crate::db::users;
use crate::stations::supervisor::ProcessState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/stations", get(list_stations).post(create_station))
        .route(
            "/api/stations/{id}",
            get(get_station).put(update_station).delete(delete_station),
        )
        .route("/api/stations/{id}/status", get(station_status))
        .route("/api/stations/{id}/cmd", post(station_cmd))
        .route("/api/stations/{id}/events", get(station_events))
        .route("/api/stations/{id}/history", get(station_history))
        .route("/api/stations/{id}/stream", get(stream_mount))
        .route("/api/webhooks/track", post(track_webhook))
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

async fn list_stations(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<Vec<Station>>> {
    Ok(Json(stations::list(&state.pool).await?))
}

async fn get_station(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Station>> {
    Ok(Json(stations::get(&state.pool, &id).await?))
}

async fn create_station(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Json(input): Json<StationInput>,
) -> ApiResult<(StatusCode, Json<Station>)> {
    if !user.can_create_stations() {
        return Err(forbidden("station_manager permission required"));
    }
    validate_hls(&input)?;
    let station = stations::create(&state.pool, &input).await?;
    // A failed engine start (bad config, missing binary) must not leave the
    // station half-created in the DB; the apply error is returned.
    state.supervisor.apply(&station).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "station.create",
        "stations",
        &format!("{} ({})", station.name, station.id),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(station)))
}

async fn update_station(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<StationInput>,
) -> ApiResult<Json<Station>> {
    if !user.can_manage_stations(&id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    validate_hls(&input)?;
    let station = stations::update(&state.pool, &id, &input).await?;
    // Atomic config swap: kill + respawn the engine with the new script.
    state.supervisor.apply(&station).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "station.update",
        "stations",
        &format!("{} ({})", station.name, station.id),
    )
    .await?;
    Ok(Json(station))
}

/// HLS needs a writable directory to slice segments into; the engine's
/// `--check` rejects `output.hls` without one, so fail early instead of
/// leaving the station in a broken config.
fn validate_hls(input: &StationInput) -> ApiResult<()> {
    if *input.hls_enabled && input.hls_dir.trim().is_empty() {
        return Err(ApiError::bad_request(
            "hls_dir is required when HLS is enabled",
        ));
    }
    Ok(())
}

async fn delete_station(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    if !user.can_manage_stations(&id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    state.supervisor.stop(&id).await?;
    // The supervisor's watchdog also fires "stopped", but it races the
    // delete below (webhook rows cascade with the station), so fire it here
    // while the station still exists.
    crate::notify::station_event(&state.pool, &id, "stopped").await;
    stations::delete(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "station.delete",
        "stations",
        &id,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct StatusBody {
    process: ProcessState,
    pid: Option<u32>,
    uptime_seconds: Option<u64>,
    restarts: u64,
    last_error: Option<String>,
    playing: Option<String>,
    engine_uptime_seconds: Option<u64>,
    engine_ok: bool,
    /// True while a live DJ holds the harbor; the playlist is ducked.
    live: bool,
    history: Vec<song_history::SongHistory>,
}

async fn station_status(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<StatusBody>> {
    let station = stations::get(&state.pool, &id).await?;
    let process = state.supervisor.status(&id).await;

    // Poll the engine control port for live status; a dead control port is
    // not fatal (the engine may still be starting).
    let client = ControlClient::new(format!("http://127.0.0.1:{}", station.control_http_port));
    let (playing, engine_uptime, live) = match client.status().await {
        Ok(s) => (Some(s.playing), Some(s.uptime_seconds), s.harbor_connected),
        Err(_) => (None, None, false),
    };

    Ok(Json(StatusBody {
        process: process.state,
        pid: process.pid,
        uptime_seconds: process.uptime_seconds,
        restarts: process.restarts,
        last_error: process.last_error,
        playing,
        engine_uptime_seconds: engine_uptime,
        engine_ok: engine_uptime.is_some(),
        live,
        history: song_history::recent(&state.pool, &id, 20).await?,
    }))
}

#[derive(Deserialize)]
struct CmdRequest {
    command: String,
}

async fn station_cmd(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(req): Json<CmdRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if !user.can_control_station(&id) {
        return Err(forbidden(
            "station_manager or dj permission required for this station",
        ));
    }
    let station = stations::get(&state.pool, &id).await?;
    let client = ControlClient::new(format!("http://127.0.0.1:{}", station.control_http_port));
    let reply = client.cmd(&req.command).await?;
    Ok(Json(json!({ "ok": reply.ok, "message": reply.error })))
}

/// SSE stream of station events (track changes + status snapshots). The
/// engine pushes track changes to the webhook; this endpoint fans them out
/// to browser clients, blending in control-port status as a keepalive.
async fn station_events(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let hub = state.hub.clone();
    let rx = hub.subscribe(&id).await;

    // First frame: current now-playing (or idle), per SSE reconnect spec.
    let now = song_history::now_playing(&state.pool, &id).await?;
    let initial = match now {
        Some(h) => sse_frame(&StationEvent::Track(TrackEvent {
            title: h.title,
            started_at: h.started_at,
        })),
        None => sse_frame(&StationEvent::Status(StatusEvent {
            state: "idle".into(),
            playing: None,
            uptime_seconds: None,
        })),
    };

    let stream = rx.map(|ev| Ok::<_, axum::Error>(Event::default().data(sse_frame(&ev))));

    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, "text/event-stream"),
            (CACHE_CONTROL, "no-cache"),
            (
                axum::http::header::HeaderName::from_static("x-accel-buffering"),
                "no",
            ),
        ],
        // Prefix the replayed current track, then live events. Status
        // blending is done by the client polling `/status` on the SSE
        // keepalive interval; the hub carries the authoritative events.
        Sse::new(
            tokio_stream::once(Ok::<_, axum::Error>(Event::default().data(initial))).chain(stream),
        )
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    ))
}

async fn station_history(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<song_history::SongHistory>>> {
    Ok(Json(song_history::recent(&state.pool, &id, 50).await?))
}

/// Engine webhook receiver: `POST /api/webhooks/track?station=<id>` with a
/// JSON body carrying the track title. The generated `crabsoup.lua` embeds
/// the station id in the URL (the engine's `http_post` sends no headers).
/// Records history and pushes to SSE.
#[derive(Deserialize)]
struct TrackPayload {
    #[serde(default)]
    title: String,
}

async fn track_webhook(
    State(state): State<AppState>,
    Query(query): Query<serde_json::Value>,
    body: axum::body::Bytes,
) -> ApiResult<Json<serde_json::Value>> {
    let station_id = query
        .get("station")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing ?station=<id> query parameter".into(),
        })?;

    let payload: TrackPayload = serde_json::from_slice(&body).map_err(|e| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid JSON body: {e}"),
    })?;

    // The engine fires metadata hooks even when no source is active (the
    // fallback momentarily has nothing to play); those carry empty or
    // "(no source)" titles and are not real tracks.
    if payload.title.trim().is_empty() || payload.title.trim() == "(no source)" {
        return Ok(Json(json!({ "ok": true, "skipped": true })));
    }
    let title = payload.title;

    let row = song_history::push(&state.pool, station_id, &title).await?;
    state
        .hub
        .publish(
            station_id,
            StationEvent::Track(TrackEvent {
                title: row.title,
                started_at: row.started_at,
            }),
        )
        .await;
    // Real audio again → clear any open dead-air alert.
    crate::api::analytics::clear_dead_air(&state.pool, station_id).await;
    Ok(Json(json!({ "ok": true })))
}

/// Reverse-proxy the live Icecast mount so the browser plays the stream
/// from the same origin (no mixed-content / CORS surprises). The upstream
/// body is streamed through chunk by chunk.
async fn stream_mount(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let station = stations::get(&state.pool, &id).await?;
    let mount = station.icecast_mount.trim_start_matches('/');
    let upstream = format!(
        "http://{}:{}/{mount}",
        station.icecast_host, station.icecast_port
    );

    let client = reqwest::Client::new();
    let res = client
        .get(&upstream)
        .header("Icy-MetaData", "0")
        .send()
        .await
        .map_err(|e| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("icecast unreachable ({upstream}): {e}"),
        })?;
    let status = res.status();
    if !status.is_success() {
        return Err(ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("icecast returned {status} for {upstream}"),
        });
    }

    let content_type = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("audio/mpeg")
        .to_string();
    let stream = res.bytes_stream();
    Ok((
        StatusCode::OK,
        [
            (
                CONTENT_TYPE,
                HeaderValue::from_str(&content_type)
                    .unwrap_or_else(|_| HeaderValue::from_static("audio/mpeg")),
            ),
            // Live audio is continuous and unbounded — never cached.
            (CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        Body::from_stream(stream),
    ))
}
