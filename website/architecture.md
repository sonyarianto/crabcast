# Architecture

```
┌─────────────────────────── crabcast ───────────────────────────┐
│  web/ (Vite + React SPA, Tailwind v4, shadcn/ui)                │
│    admin UI · public pages · embeddable player widget           │
│        │  REST + SSE (proxied /api: dev vite proxy, prod nginx) │
│  server/ (Rust, axum + tokio + sqlx)                            │
│    api/            REST routes + SSE hub                        │
│    auth/           sessions, roles, permissions, API tokens     │
│    stations/       station lifecycle: spawn/restart/health      │
│    lua/            DB config → crabsoup.lua generator           │
│    control/        Crabsoup HTTP control client                 │
│    media/          upload, tag scan, cover art, waveforms       │
│    analytics/      Icecast admin polling, alerts, retention     │
│    db/             SQLite (sqlx), migrations                    │
│        │  spawn one process per station                         │
│        ▼                                                         │
│  crabsoup engine (sibling repo)  ──►  Icecast  ──►  listeners   │
│    playlist/schedule/jingles/live harbor/mounts                  │
└─────────────────────────────────────────────────────────────────┘
```

## Design rules

- **The Rust API is the single source of truth.** The web app never touches
  the DB; everything goes through `/api/*` with relative paths — a dev
  proxy in `web/vite.config.ts` and nginx in production forward `/api` to
  the API. One auth model.
- **One engine process per station.** The supervisor generates
  `crabsoup.lua` from the DB, validates it with `crabsoup --check`, then
  spawns/restarts the engine with exponential backoff. Config changes are
  atomic: write → check → swap → restart.
- **Push, don't poll.** The engine posts track changes to a webhook
  (`on_metadata` → `/api/webhooks/track`); the API records history and
  fans events out over SSE. The dashboard blends in control-port status as
  a keepalive.
- **Media is content-addressed.** Uploads dedupe by sha256, files shard
  under the storage root, cover art is extracted once and served
  immutable-cacheable.

## Station lifecycle

1. Create/update a station in the DB → the supervisor re-renders the Lua
   script and `--check`s it against the real engine.
2. If valid, the old engine (if any) is stopped and the new one spawned
   with logs captured to `CRABCAST_DATA_DIR/logs/<id>.log`.
3. The watchdog polls the child; on crash it restarts with backoff (max
   30 s), tracking consecutive short-lived crashes for the
   `engine_crash_loop` alert.

## Engine integration

Crabsoup is a sibling repo consumed as a path dependency / released
binary. Integration points:

- **HTTP control API** (`server.telnet({http_port = N})`): `GET /status`,
  `/uptime`, `/queue`, `/jingles`; `POST /cmd` — one JSON envelope
  `{"ok": true, ...}`.
- **Lua generation**: playlists → `playlist`/`request.dynamic`, dayparting
  → `switch`, mounts → `output.icecast`, jingles → `jingles`, harbor →
  `input.harbor` (with per-DJ `extra_passwords`), dead air →
  `blank.detect` with an `on_blank` webhook.
- **Process supervision**: backend-owned lifecycle per station; health via
  `/status`.

## Repository layout

```
server/          Rust API (axum, SQLite, migrations, benches)
web/             Vite + React SPA (admin, public pages, widget)
website/         This documentation site (VitePress)
docker/          Dockerfiles + compose.yml (server + web + icecast)
scripts/         load-test.sh, bench-station.sh
ROADMAP.md       The plan (phases, SLOs, decision log)
```
