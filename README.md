# Crabcast

An AzuraCast-style web radio management platform — multi-station, playlist
automation, live DJ support, listener requests, analytics, and beautiful
admin/public UIs — built on a **Rust** backend and the
[Crabsoup](https://github.com/) streaming engine, with a **Next.js 16** +
**Tailwind CSS v4** + **shadcn/ui** frontend.

See [ROADMAP.md](ROADMAP.md) for the full plan and current phase.

## Stack

| Layer | Choice |
| --- | --- |
| Engine | Crabsoup (Rust) |
| Backend | Rust, axum + tokio + sqlx |
| DB | SQLite (Postgres later) |
| Frontend | Next.js 16 (App Router, Turbopack), Tailwind v4, shadcn/ui |
| Streaming | Icecast (initially) |

## Quickstart

### Local (no Docker)

```sh
# 1. API server on :8080
cargo run --manifest-path server/Cargo.toml

# 2. Web app on :3000 (proxies /api/* to the API)
npm --prefix web run dev
```

Open http://localhost:3000 — the home page shows a live API health check.

### Docker

```sh
make dev   # full stack: server + web + icecast
```

## Commands

```sh
make dev    # full stack via docker compose
make lint   # clippy -D warnings + eslint
make test   # cargo test
make fmt    # cargo fmt + prettier
```

## Layout

```
server/   Rust API (axum, SQLite, migrations)
web/      Next.js 16 app (admin, public pages, player widget)
docker/   Dockerfiles + compose.yml
```

## Conventions

- The Rust API is the single source of truth; the web app talks to it via
  Next rewrites (`/api/*`), never directly to the DB.
- `cargo fmt`/`clippy -D warnings` must stay clean; CI enforces it.
- Every station config change is validated with `crabsoup --check` before
  apply (Phase 1+).
