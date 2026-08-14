//! Playlist routes (Phase 4): per-station playlists with ordered tracks,
//! per-track fade/cue overrides, daypart schedules, and a live preview of
//! the generated Lua. Every mutation re-applies the station's engine config
//! so changes go live without manual restarts.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::playlists::{self, PlaylistDetail, ScheduleInput, TrackOverrides};
use crate::db::stations;
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/playlists",
            axum::routing::get(list_playlists).post(create_playlist),
        )
        .route(
            "/api/stations/{station_id}/playlists/preview",
            axum::routing::get(preview_lua),
        )
        .route(
            "/api/playlists/{id}",
            axum::routing::get(get_playlist)
                .put(update_playlist)
                .delete(delete_playlist),
        )
        .route("/api/playlists/{id}/tracks", post(add_tracks))
        .route("/api/playlists/{id}/tracks/reorder", put(reorder_tracks))
        .route(
            "/api/playlists/{id}/tracks/{media_id}",
            put(update_track).delete(remove_track),
        )
        .route("/api/playlists/{id}/schedules", post(add_schedule))
        .route(
            "/api/playlists/{id}/schedules/{schedule_id}",
            delete(delete_schedule),
        )
}

fn forbidden(msg: &str) -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: msg.into(),
    }
}

/// Re-render + restart the station's engine after a playlist mutation, so
/// the change takes effect immediately. Fails are reported (the DB change
/// stands; the supervisor surfaces the crash-loop state via last_error).
async fn reapply(state: &AppState, station_id: &str) -> ApiResult<()> {
    let station = stations::get(&state.pool, station_id).await?;
    state.supervisor.apply(&station).await?;
    Ok(())
}

async fn list_playlists(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<Vec<PlaylistDetail>>> {
    Ok(Json(
        playlists::detail_for_station(&state.pool, &station_id).await?,
    ))
}

async fn get_playlist(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PlaylistDetail>> {
    Ok(Json(playlists::detail(&state.pool, &id).await?))
}

async fn create_playlist(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    Json(input): Json<playlists::PlaylistInput>,
) -> ApiResult<(StatusCode, Json<playlists::Playlist>)> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let playlist = playlists::create(&state.pool, &station_id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.create",
        "playlists",
        &format!("{} ({})", playlist.name, playlist.id),
    )
    .await?;
    // A broken config must not leave the station half-applied; apply errors
    // bubble up. An empty playlist is fine (nothing to play yet).
    reapply(&state, &station_id).await?;
    Ok((StatusCode::CREATED, Json(playlist)))
}

async fn update_playlist(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<playlists::PlaylistInput>,
) -> ApiResult<Json<playlists::Playlist>> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let updated = playlists::update(&state.pool, &id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.update",
        "playlists",
        &format!("{} ({id})", updated.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(Json(updated))
}

async fn delete_playlist(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    playlists::delete(&state.pool, &id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.delete",
        "playlists",
        &format!("{} ({id})", playlist.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct AddTracksBody {
    media_ids: Vec<String>,
}

async fn add_tracks(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<AddTracksBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let added = playlists::add_tracks(&state.pool, &id, &body.media_ids).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.add_tracks",
        "playlists",
        &format!("{} track(s) → {}", added, playlist.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(Json(json!({ "added": added })))
}

#[derive(Deserialize)]
struct ReorderBody {
    media_ids: Vec<String>,
}

async fn reorder_tracks(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<ReorderBody>,
) -> ApiResult<StatusCode> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    playlists::reorder(&state.pool, &id, &body.media_ids).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.reorder",
        "playlists",
        &playlist.name,
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_track(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((id, media_id)): Path<(String, String)>,
    Json(overrides): Json<TrackOverrides>,
) -> ApiResult<StatusCode> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    playlists::update_track_overrides(&state.pool, &id, &media_id, &overrides).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.update_track",
        "playlists",
        &format!("overrides → {media_id} in {}", playlist.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_track(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((id, media_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    playlists::remove_track(&state.pool, &id, &media_id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.remove_track",
        "playlists",
        &format!("{media_id} ← {}", playlist.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_schedule(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(id): Path<String>,
    Json(input): Json<ScheduleInput>,
) -> ApiResult<(StatusCode, Json<playlists::PlaylistSchedule>)> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    let schedule = playlists::add_schedule(&state.pool, &id, &input).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.add_schedule",
        "playlists",
        &format!(
            "{} → {} ({} to {})",
            playlist.name, input.days, input.start_time, input.end_time
        ),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok((StatusCode::CREATED, Json(schedule)))
}

async fn delete_schedule(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path((id, schedule_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let playlist = playlists::get(&state.pool, &id).await?;
    if !user.can_manage_stations(&playlist.station_id) {
        return Err(forbidden(
            "station_manager permission required for this station",
        ));
    }
    playlists::delete_schedule(&state.pool, &schedule_id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "playlist.delete_schedule",
        "playlists",
        &format!("schedule {schedule_id} ← {}", playlist.name),
    )
    .await?;
    reapply(&state, &playlist.station_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Live preview of the Lua that would be generated for the station's
/// playlists — the scheduler UI shows this before rules are saved.
async fn preview_lua(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let station = stations::get(&state.pool, &station_id).await?;
    let media_root = state.storage.root();
    let playlists = playlists::sources(&state.pool, &station_id, media_root).await?;
    let mut script = String::new();
    crate::lua::render_playlist_sources(&mut script, &playlists, &station.playlist_dir);
    Ok(Json(json!({ "lua": script })))
}
