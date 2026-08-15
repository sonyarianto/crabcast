# Scaling & multi-host deployment

Crabcast is built so the control plane can grow from a single VPS to
several machines without rewriting anything. This page lays out the
deployment models, what works today, and what unlocks the next tier.

## The moving parts

- **API server** (Rust, axum): the control plane — auth, stations, media,
  playlists, requests, analytics, backup. It supervises one **station
  engine** (`crabsoup`) process per station.
- **Station engines**: one `crabsoup` per station, spawned by the API
  process on the same host, streaming to Icecast.
- **Web app** (Next.js): stateless; calls the API through rewrites. Scale
  it freely behind a load balancer — it holds no state of its own.
- **Event bus**: station events (track changes for the SSE dashboards) fan
  out through an in-process hub by default, or through **Redis pub/sub**
  when `CRABCAST_REDIS_URL` is set on every API host.
- **Database**: the API's single source of truth. **SQLite** today
  (embedded, zero-ops); a **Postgres** backend is the planned unlock for
  multi-writer deployments (see below).

## Deployment models

### 1. Single host (default — `docker compose up`)

One API process + one web process + Icecast + SQLite on a shared volume.
The supervisor's engines run in the same container as the API. This is the
model the install script and `docker/compose.prod.yml` target, and it
comfortably runs dozens of stations on a small VPS (see the benchmark
table in the ROADMAP).

### 2. Web replicas (works today)

The web app is stateless, so run as many copies as you like behind a
reverse proxy. The single API stays the only writer to SQLite. This covers
traffic spikes without touching the control plane.

### 3. Multiple API hosts (needs Postgres)

This is the full horizontal-scale story: **N API hosts, M station hosts,
one shared database**, with Redis carrying the event bus so every
dashboard sees every station no matter which host handled the track
webhook.

What's already in place:

- **Redis event bus** — set `CRABCAST_REDIS_URL` on every API host and the
  SSE hubs share one pub/sub channel per station. Verified: a track change
  reported to host A reaches a dashboard connected to host B.
- **REST API + API tokens** — scripts and replicas authenticate with
  `Authorization: Bearer cb_…` tokens instead of browser sessions, so a
  replica can call any host.
- **Stateless handlers** — every request is a DB round-trip; there is no
  per-host in-memory state that a second host would miss.

What it still needs before it's safe:

- **Postgres backend** — SQLite is an embedded single-writer database;
  multiple API processes cannot share one SQLite file reliably. The
  ROADMAP's Postgres feature flag is the planned dual-driver backend
  (`sqlx` already supports both). Until it lands, run exactly **one** API
  host and scale the web tier instead.
- **Session store** — browser sessions are stored in the API's database,
  so they are per-host. With Postgres the session store moves into the
  shared DB automatically; today, a load balancer must pin a user to one
  API host (sticky sessions) or rely on API tokens.
- **Station placement** — station engines live on the API host that
  spawned them, reading the shared media volume. In the M-station-host
  model the engines run on dedicated hosts that can see the media (shared
  NFS volume or replicated media), and the supervisor placement is
  delegated — the engine binary and media must be present on whichever
  host runs a station.

## Target topology (once Postgres lands)

```
                 ┌──────────────┐
   users ───────▶│  web replica │──┐
                 └──────────────┘  │   ┌──────────────────┐
                                   ├──▶│  API host 1      │──┐
                 ┌──────────────┐  │   │  (supervisor)    │  │
   users ───────▶│  web replica │──┼──▶├──────────────────┤  │
                 └──────────────┘  │   │  API host N      │──┼──▶ Postgres (shared)
                                   └──▶├──────────────────┤  │
                 ┌──────────────┐      │  station hosts   │  │
   DJs ─────────▶│  Icecast     │──────┘  (one per engine)│──┘
                 └──────────────┘      └──────────────────┘
                        ▲                      ▲
                        └──────── Redis pub/sub (event bus) ─┘
```

- Web replicas: stateless, any number.
- API hosts: one shared DB (Postgres), one shared event bus (Redis).
  Write-heavy work (media uploads, backups) can be pinned to one host.
- Station hosts: run the `crabsoup` engines, one per station, with access
  to the media volume; the API supervises them over the control ports.
- Icecast: one or more instances; engines push to whatever mount/instance
  the station is configured with.

## What to do today

1. Run the single-host stack (`docker compose -f docker/compose.prod.yml up -d --build`).
2. Add a reverse proxy (Caddy/nginx) in front of web + API, set
   `CRABCAST_SESSION_SECRET` once, and put Redis behind `CRABCAST_REDIS_URL`
   if you run any second API process for read-only tooling.
3. Watch the ROADMAP for the Postgres backend — it flips the multi-host
   model from "planned" to "supported".
