# v0.1.0 release checklist

Crabcast is an AzuraCast-style self-hosted web radio platform: a Rust API
that supervises one [Crabsoup](https://github.com/sonyarianto/crabsoup)
engine per station, streaming to Icecast (and optionally HLS), with a
Vite/React admin, public pages, and an embeddable widget.

This doc is the operator's manual for deploying and verifying a release.
The roadmap is in [ROADMAP.md](../ROADMAP.md); history in
[CHANGELOG.md](../CHANGELOG.md).

## 1. Prerequisites

- The **Crabsoup engine binary** (`crabsoup` ≥ 0.1.0) — install it or point
  `CRABSOUP_BIN` at it.
- **Icecast** reachable from the API host (the Docker quickstart includes
  one).
- **Node.js 22+** only needed to build the web app; the runtime is static.

## 2. Environment variables

| Variable | Required | Default | Purpose |
| --- | --- | --- | --- |
| `DATABASE_URL` | yes | — | SQLite path, e.g. `sqlite:/data/crabcast.db` |
| `BIND_ADDR` | no | `127.0.0.1:8080` | API listen address |
| `CRABCAST_DATA_DIR` | yes | — | Station configs/logs (`configs/<id>/`, `logs/<id>.log`) |
| `CRABCAST_MEDIA_DIR` | yes | — | Media library root |
| `CRABCAST_SESSION_SECRET` | yes | — | Session signing secret (`openssl rand -hex 32`) |
| `CRABSOUP_BIN` | no | `crabsoup` on PATH | Engine binary |
| `CRABCAST_WEBHOOK_URL` | no | `http://localhost:8080/api/webhooks/track` | Track-webhook base URL baked into `crabsoup.lua`; set it when the API isn't on localhost:8080 (the dead-air webhook is not overridable) |
| `CRABCAST_ALERT_WEBHOOK_URL` | no | — | Crash-loop alert webhook (Slack/Discord generic JSON) |
| `CRABCAST_REDIS_URL` | no | — | Redis pub/sub SSE bus for multi-API-host setups |
| `CRABCAST_RETENTION_DAYS` | no | 30 | Analytics history retention |
| `API_UPSTREAM` | no | `http://localhost:8080` | Web app's API origin (vite dev proxy / nginx envsubst) |

## 3. Deploy

Two supported paths (details in `packaging/README.md` and
`website/guide/scaling.md`):

- **Docker**: `./scripts/install.sh` or
  `docker compose -f docker/compose.prod.yml up -d --build` (API + web +
  Icecast; secrets via environment).
- **Bare metal (systemd)**: build `server` + `web`, install the two units
  in `packaging/`, and serve the SPA with nginx or `web/serve.mjs`.

## 4. Upgrade from an older build

1. **Back up first** (admin UI → Settings → Backup & restore, or the
   `scripts/`-less API: `GET /api/backup/download` as super admin).
2. Stop the stack (`docker compose ... down` or
   `sudo systemctl stop crabcast-server crabcast-web`).
3. Replace the binaries/images. **Migrations run automatically at boot** —
   no manual SQL.
4. Start the stack and run the verification checklist below.

## 5. Verification checklist

- [ ] `GET /api/health` returns `{"status":"ok","db":"ok"}`.
- [ ] First boot shows the bootstrap screen; create the super admin.
- [ ] Upload audio → it appears in the Library with tags + cover art.
- [ ] Create a station → engine spawns (`GET /api/stations/{id}` status
      `running`), audio reaches Icecast.
- [ ] Playlist automation: add a playlist, confirm tracks rotate.
- [ ] HLS: enable it in the station profile with a writable directory →
      `playlist.m3u8` + `seg-*.ts` appear; the public page plays it.
- [ ] Notifications: add a Slack/Discord webhook in the station page →
      receive `started` on restart, `crashed` on engine kill.
- [ ] Podcasts: publish an episode → the RSS feed at
      `/api/public/stations/{id}/podcast.rss` parses.
- [ ] Backup: download a backup, wipe, restore → the server restarts and
      data is back.
- [ ] Widget: the embed page (`/stations/{id}/widget`) shows the player.
- [ ] PWA: install the admin from a phone; station controls work.

## 6. Known limitations (post-1.0 roadmap)

- **SQLite is single-writer** — run one API host per database; multi-host
  wants the Postgres backend (deferred).
- **Icecast is required** for the classic mount stream (built-in mount
  server is the last Phase 11 stretch).
- HLS is standard (6s segments), not LL-HLS yet.
- i18n (non-English UI) is not shipped yet.
