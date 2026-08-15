# REST API & tokens

The Rust API is the single source of truth. The web app talks to it through
Next rewrites (`/api/*` → `API_UPSTREAM`); third-party clients can hit the
API directly.

- Base URL: `http://<host>:8080` (or the proxied origin in production).
- Errors: non-2xx responses carry `{"error": "..."}`.
- Auth: session cookies for the web UI, **Bearer tokens** for scripts.

## Authentication

### API tokens

Create a token in the **Settings** page, or via the API:

```sh
# create (the secret is shown exactly once)
curl -X POST $API/api/tokens \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"ci-script"}'
# → { "id": "...", "name": "ci-script", "secret": "cb_..." , ... }

curl $API/api/tokens                      # list my tokens
curl -X DELETE $API/api/tokens/$ID        # revoke (immediate)
```

Then authenticate every request with `Authorization: Bearer cb_...`.
Tokens inherit your account's permissions. A revoked or invalid bearer is
rejected with 401 — it never silently falls back to a session.

### Session auth

- `POST /api/auth/setup` — first-run detection: `{"needed": true}`.
- `POST /api/auth/bootstrap` — create the initial admin (only while the
  users table is empty).
- `POST /api/auth/login` / `POST /api/auth/logout` — session cookie auth.
- `GET /api/auth/me` — current user, role grants, and the CSRF token.
- `POST /api/auth/password` — change password (needs `current_password`).

All mutations from the web UI send the session CSRF token in
`X-CSRF-Token` (ignored for Bearer-authenticated requests).

## Endpoints

### System

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/api/health` | `{"status":"ok","db":"ok"}` — no auth |

### Stations & control

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/stations` | List / create (manager) |
| GET/PUT/DELETE | `/api/stations/{id}` | Full station model |
| GET | `/api/stations/{id}/status` | Process state, now playing, `live` (DJ on air) |
| POST | `/api/stations/{id}/cmd` | `skip`, `jingles.play`, `queue.*` |
| GET | `/api/stations/{id}/events` | SSE stream (track changes) |
| GET | `/api/stations/{id}/history` | Recent song history |
| GET | `/api/stations/{id}/stream` | Reverse-proxied live mount (same-origin playback) |

### Media library

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/media` | List (search/facets/sort/paginate) / upload (multipart, dedupe by sha256) |
| GET | `/api/media/facets` | artist / album / genre facets |
| GET | `/api/media/config` | Storage root (point a playlist dir at it) |
| GET/PUT/DELETE | `/api/media/{id}` | Detail / tag edit (writes back to the file) / delete |
| GET | `/api/media/{id}/stream` | Range-enabled audio (ETag + 304) |
| GET | `/api/media/{id}/cover` | Cover art (immutable-cacheable) |

### Playlists & scheduling

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/stations/{station_id}/playlists` | List / create |
| GET | `/api/stations/{station_id}/playlists/preview` | Generated Lua |
| PUT/DELETE | `/api/playlists/{id}` | Update / delete |
| POST | `/api/playlists/{id}/tracks` | Add tracks (`media_ids`) |
| PUT | `/api/playlists/{id}/tracks/reorder` | Order by `media_ids` |
| PUT/DELETE | `/api/playlists/{id}/tracks/{media_id}` | Fade/cue overrides / remove |
| POST/DELETE | `/api/playlists/{id}/schedules[/{schedule_id}]` | Daypart rules |

### Streamers (live DJs)

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/stations/{station_id}/streamers` | List / create (per-DJ source password) |
| PUT/DELETE | `/api/streamers/{id}` | Update / disable |
| GET | `/api/streamers/{id}/connect` | Mount URL, credentials, `curl` mic test |

### Requests & jingles

| Method | Path | Notes |
| --- | --- | --- |
| GET/PUT | `/api/stations/{station_id}/request-rules` | enabled / max_per_hour / dedupe / moderation |
| POST | `/api/stations/{station_id}/requests` | Anonymous request (`media_id`) |
| GET | `/api/stations/{station_id}/requests?pending=` | History / moderation inbox |
| POST | `/api/stations/{station_id}/requests/{id}/approve` · `/reject` | Moderation |
| GET/POST | `/api/stations/{station_id}/queue` | Engine queue view / clear |
| POST | `/api/stations/{station_id}/queue/skip` | Skip track |
| GET/POST | `/api/stations/{station_id}/jingles` | List / upload |
| DELETE | `/api/stations/{station_id}/jingles/{filename}` | Delete |

### Analytics & alerts

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/api/stations/{id}/analytics/listeners?from&to&bucket` | Bucketed listener series |
| GET | `/api/stations/{id}/analytics/summary` | Listeners now, unique 24h, uptime %, plays/requests today |
| GET | `/api/stations/{id}/analytics/top-songs?days` | Plays + air time |
| GET | `/api/stations/{id}/analytics/requests?days` | Per-day request stats |
| GET | `/api/stations/{id}/analytics/history.csv?days` | Song-history CSV export |
| GET | `/api/alerts?station_id=&open=` | Alert feed (global view: super admin) |
| POST | `/api/alerts/{id}/resolve` | Resolve (station manager) |

### Podcasts

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/stations/{station_id}/podcasts` | List / publish an episode (station manager) |
| DELETE | `/api/podcasts/{episode_id}` | Delete an episode |

Episode audio references a media-library file (`media_id`); the public
RSS feed is rendered from the same table.

### Admin

| Method | Path | Notes |
| --- | --- | --- |
| GET/POST | `/api/users` | User CRUD (super admin) |
| PUT/DELETE | `/api/users/{id}` | Update / delete |
| GET | `/api/audit` | Audit log (who changed what) |

## Public surface (no auth)

Used by the public page, the embeddable widget, and third-party clients:

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/api/now-playing` | Every station with now playing + history |
| GET | `/api/station/{id}/now-playing` | Single station (same payload) |
| GET | `/api/public/stations/{station_id}` | Brand, socials, requests flag, stream URL |
| GET | `/api/public/stations/{station_id}/library?q=` | Lightweight search for the request form |
| POST | `/api/stations/{station_id}/requests` | Anonymous listener request |
| GET | `/api/public/stations/{station_id}/podcast.rss` | RSS 2.0 podcast feed (episodes, enclosure) |

```sh
curl $API/api/now-playing
# → [{ "id": "...", "name": "Test FM", "stream_url": "/api/stations/.../stream",
#      "now": {"title": "Artist - Song", "started_at": "..."}, "history": [...] }]
```

## Engine webhooks (internal)

The generated `crabsoup.lua` pings these — used by the supervisor, not by
clients:

- `POST /api/webhooks/track?station=<id>` — track-change events (records
  history, pushes SSE, clears dead-air alerts).
- `POST /api/webhooks/blank?station=<id>` — dead-air episodes (raises a
  `dead_air` alert).
