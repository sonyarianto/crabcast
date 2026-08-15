//! Podcast episodes (Phase 11 stretch): per-station episode CRUD for
//! station managers, plus a public RSS 2.0 feed (`/api/public/stations/
//! {id}/podcast.rss`) that podcast apps can subscribe to. Episodes
//! reference audio files already in the media library.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth::{Csrf, CurrentUser};
use crate::db::podcasts::{self, Episode};
use crate::db::users;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/stations/{station_id}/podcasts",
            get(list).post(create),
        )
        .route("/api/podcasts/{episode_id}", axum::routing::delete(delete))
        .route(
            "/api/public/stations/{station_id}/podcast.rss",
            get(rss_feed),
        )
}

#[derive(Deserialize)]
struct EpisodeInput {
    media_id: String,
    title: String,
    #[serde(default)]
    description: String,
}

async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(station_id): Path<String>,
) -> ApiResult<Json<Vec<Episode>>> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    let rows = podcasts::list(&state.pool, &station_id).await?;
    Ok(Json(rows.into_iter().map(|r| r.into_episode()).collect()))
}

async fn create(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(station_id): Path<String>,
    Json(input): Json<EpisodeInput>,
) -> ApiResult<(StatusCode, Json<Episode>)> {
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    let title = input.title.trim();
    if title.is_empty() {
        return Err(ApiError::bad_request("episode title is required"));
    }
    let episode = podcasts::create(
        &state.pool,
        &station_id,
        &input.media_id,
        title,
        &input.description,
    )
    .await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "podcast.create",
        &episode.id,
        &format!("station {station_id}: {}", episode.title),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(episode)))
}

async fn delete(
    State(state): State<AppState>,
    _csrf: Csrf,
    user: CurrentUser,
    Path(episode_id): Path<String>,
) -> ApiResult<StatusCode> {
    // The episode's station is the scope for the permission check.
    let station_id =
        sqlx::query_scalar::<_, String>("SELECT station_id FROM podcast_episodes WHERE id = $1")
            .bind(&episode_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::not_found("podcast episode", &episode_id))?;
    if !user.can_manage_stations(&station_id) {
        return Err(forbidden());
    }
    podcasts::delete(&state.pool, &episode_id).await?;
    users::log_audit(
        &state.pool,
        Some(&user.user.id),
        "podcast.delete",
        &episode_id,
        &format!("station {station_id}"),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Public RSS 2.0 feed. Episodes are ordered newest first; the enclosure
/// points at the media stream endpoint (same-origin absolute URL built
/// from the Host header).
async fn rss_feed(
    State(state): State<AppState>,
    Path(station_id): Path<String>,
    req: axum::http::Request<axum::body::Body>,
) -> ApiResult<Response> {
    let station = crate::db::stations::get(&state.pool, &station_id).await?;
    let episodes = podcasts::list(&state.pool, &station_id).await?;

    let base = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8080");
    let base_url = format!("http://{base}");

    let xml = render_feed(
        &station.name,
        &station.description,
        &base_url,
        &station_id,
        &episodes,
    );
    Ok((
        StatusCode::OK,
        [
            (
                CONTENT_TYPE,
                HeaderValue::from_static("application/rss+xml; charset=utf-8"),
            ),
            (
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache"),
            ),
        ],
        xml,
    )
        .into_response())
}

/// RSS 2.0 with a minimal iTunes namespace (author/description only).
fn render_feed(
    station_name: &str,
    station_description: &str,
    base_url: &str,
    station_id: &str,
    episodes: &[podcasts::EpisodeFeedRow],
) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <rss version=\"2.0\" xmlns:itunes=\"http://www.itunes.com/dtds/podcast-1.0.dtd\" \
         xmlns:atom=\"http://www.w3.org/2005/Atom\">\n<channel>\n",
    );
    s.push_str(&format!("<title>{}</title>\n", escape_xml(station_name)));
    s.push_str(&format!(
        "<link>{base_url}</link>\n<description>{}</description>\n",
        escape_xml(station_description)
    ));
    s.push_str(&format!(
        "<itunes:author>{}</itunes:author>\n",
        escape_xml(station_name)
    ));
    s.push_str(&format!(
        "<atom:link href=\"{base_url}/api/public/stations/{station_id}/podcast.rss\" rel=\"self\" type=\"application/rss+xml\"/>\n",
    ));
    for ep in episodes {
        s.push_str("<item>\n");
        s.push_str(&format!("<title>{}</title>\n", escape_xml(&ep.title)));
        s.push_str(&format!(
            "<description>{}</description>\n",
            escape_xml(&ep.description)
        ));
        let pub_date = rfc2822(&ep.created_at);
        if !pub_date.is_empty() {
            s.push_str(&format!("<pubDate>{pub_date}</pubDate>\n"));
        }
        s.push_str(&format!(
            "<guid isPermaLink=\"false\">{}</guid>\n",
            escape_xml(&ep.id)
        ));
        let mime = if ep.mime.is_empty() {
            "audio/mpeg"
        } else {
            &ep.mime
        };
        s.push_str(&format!(
            "<enclosure url=\"{base_url}/api/media/{}/stream\" length=\"{}\" type=\"{}\"/>\n",
            ep.media_id, ep.size_bytes, mime
        ));
        s.push_str("</item>\n");
    }
    s.push_str("</channel>\n</rss>\n");
    s
}

/// RFC 3339 (`2026-08-15T06:52:44.123Z`) → RSS RFC 2822 pubDate.
/// Returns empty on any parse/format failure (a bad timestamp must not
/// take the whole feed down).
fn rfc2822(created_at: &str) -> String {
    let Ok(dt) =
        time::OffsetDateTime::parse(created_at, &time::format_description::well_known::Rfc3339)
    else {
        return String::new();
    };
    dt.format(&time::format_description::well_known::Rfc2822)
        .unwrap_or_default()
}

/// XML-escape text content (title/description/id).
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn forbidden() -> ApiError {
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: "station_manager permission required".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::podcasts::EpisodeFeedRow;

    #[test]
    fn xml_escaping_covers_specials() {
        assert_eq!(
            escape_xml("a<b&c>d\"e'f"),
            "a&lt;b&amp;c&gt;d&quot;e&apos;f"
        );
        assert_eq!(escape_xml("plain"), "plain");
    }

    #[test]
    fn rfc2822_renders_pubdate() {
        let out = rfc2822("2026-08-15T06:52:44.123456789Z");
        assert!(out.starts_with("Sat, 15 Aug 2026"), "{out}");
        assert!(out.contains("+0000"), "{out}");
    }

    fn row(id: &str, title: &str) -> EpisodeFeedRow {
        EpisodeFeedRow {
            id: id.into(),
            station_id: "s1".into(),
            media_id: "m1".into(),
            title: title.into(),
            description: "desc".into(),
            created_at: "2026-08-15T06:52:44.123456789Z".into(),
            filename: "ep.mp3".into(),
            mime: "audio/mpeg".into(),
            size_bytes: 1234,
            artist: String::new(),
            album: String::new(),
        }
    }

    #[test]
    fn feed_has_channel_and_enclosure() {
        let feed = render_feed(
            "Test & FM",
            "My <station>",
            "http://radio.example",
            "s1",
            &[row("e1", "Ep 1")],
        );
        assert!(feed.contains(
            "<atom:link href=\"http://radio.example/api/public/stations/s1/podcast.rss\""
        ));
        assert!(feed.contains("<rss version=\"2.0\""));
        assert!(feed.contains("<title>Test &amp; FM</title>"));
        assert!(feed.contains("<description>My &lt;station&gt;</description>"));
        assert!(feed.contains("<enclosure url=\"http://radio.example/api/media/m1/stream\" length=\"1234\" type=\"audio/mpeg\"/>"));
        assert!(feed.contains("<guid isPermaLink=\"false\">e1</guid>"));
    }
}
