# Getting started

Crabcast is an AzuraCast-style web radio management platform: a **Rust** API
server that supervises one [Crabsoup](https://github.com/sonyarianto/crabsoup)
engine process per station, broadcasting to **Icecast**. The admin UI is a
Next.js 16 app that talks to the API only (no direct DB access).

## Requirements

- Rust (2024 edition toolchain) and Node.js 22+
- The Crabsoup engine binary (`CRABSOUP_BIN`, default `crabsoup` on PATH)
- An Icecast server (the docker quickstart includes one)
- Audio files for your station's playlist

## Quickstart

### Docker (full stack)

```sh
make dev
```

This runs the API (`:8080`), the web app (`:3000`) and Icecast
(`:8000`), plus mounts a `media/` directory for uploads.

### Manual

```sh
# 1. API server on :8080 (runs migrations at boot)
cargo run --manifest-path server/Cargo.toml

# 2. Web app on :3000 (proxies /api/* to the API)
npm --prefix web run dev
```

Open `http://localhost:3000`. The first visit runs the **bootstrap wizard**:
create the initial admin account (email-less, argon2-hashed).

## First station in five steps

1. **Log in** as the admin you just created.
2. **Create a station** — point `playlist_dir` at a directory of audio
   files and `jingles_dir` at a directory of short clips; set the Icecast
   mount (`/radio`, `source`/password) and a control-port pair.
3. The supervisor writes `crabsoup.lua`, validates it with
   `crabsoup --check`, and spawns the engine — the station starts playing
   on the Icecast mount.
4. **Playlists** (`/stations/:id/playlists`): add tracks from the library,
   set weights and daypart schedules; changes apply live.
5. **Streamers / requests / jingles**: see the [radio operation
   guide](/guide/stations) and [requests & jingles](/guide/requests-jingles).

## Configuration (environment variables)

| Variable | Default | Purpose |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite:crabcast.db` | SQLite database file |
| `BIND_ADDR` | `0.0.0.0:8080` | API listen address |
| `CRABCAST_DATA_DIR` | `station-data` | Per-station configs + logs |
| `CRABCAST_MEDIA_DIR` | `media` | Uploaded media + cover art |
| `CRABSOUP_BIN` | `crabsoup` | Engine binary path |
| `CRABCAST_SESSION_SECRET` | random | Session-cookie encryption key (set it in prod!) |
| `CRABCAST_WEBHOOK_URL` | `http://localhost:8080/api/webhooks/track` | Engine track-change webhook |
| `CRABCAST_ALERT_WEBHOOK_URL` | — | Optional outbound alert webhook (raise/resolve events) |
| `CRABCAST_RETENTION_DAYS` | `30` | Analytics retention: listener samples, history, resolved alerts |

## Command reference

```sh
make dev        # full stack via docker compose
make lint       # clippy -D warnings + eslint
make test       # cargo test
make fmt        # cargo fmt + prettier
```
