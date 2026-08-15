# Changelog

All notable changes to Crabcast. See [ROADMAP.md](ROADMAP.md) for the full
phase-by-phase detail and decision log; this file is the condensed history.

## [0.1.0] — 2026-08-15

First release: a complete AzuraCast-style self-hosted radio platform.

### Added (by phase)

- **Phase 0 — Scaffold**: monorepo (`server/` Rust + axum + SQLite, `web/`
  Vite + React SPA, `website/` VitePress docs), `docker/compose.yml` +
  `make dev`, GitHub Actions CI.
- **Phase 1 — Control plane**: `crabsoup.lua` generator (validated with
  `crabsoup --check`, atomic swap), per-station engine supervisor (spawn,
  backoff restart, log capture, boot start-all), control client
  (`/status`, `/uptime`, `/queue`, `/jingles`, `/cmd`), track webhook →
  song history, SSE hub, station CRUD + live dashboard.
- **Phase 3 — Media library**: upload with content-hash dedupe, tag scan
  (title/artist/album/genre, duration, replaygain, cover art), waveform
  peaks, search/filters/pagination, streaming preview with Range support.
- **Phase 4 — Playlists & schedules**: playlist CRUD with per-track
  overrides, `scheduled` dayparted playlists, `once_per_hour` rotation,
  jingles directory, dead-air guard (blank.detect + alert webhook).
- **Phase 5 — Live DJs**: harbor config with per-DJ passwords, live/auto
  switching, streamer management + connect info.
- **Phase 6 — Listener requests**: request queue (approve/reject/skip),
  per-station rules (max/hour, dedupe, moderation), public request form.
- **Phase 7 — Public pages & widget**: public station page (now playing,
  history, request form, player), embeddable widget, library search API.
- **Phase 8 — Analytics**: icecast polling, dead-air detection + alerts,
  CSV export, retention (`CRABCAST_RETENTION_DAYS`).
- **Phase 9 — Scale**: REST API + bearer tokens, benchmarks/load tests,
  CDN-friendly media serving, Redis pub/sub SSE bus
  (`CRABCAST_REDIS_URL`), scaling guide.
- **Phase 10 — Production & backup**: multi-stage Docker images +
  compose.prod.yml + install script + systemd units, backup/restore
  (SQLite snapshot + media, validated restore at restart), onboarding
  wizard.
- **Phase 11 — Stretch goals**: podcasts (episode CRUD + public RSS 2.0
  feed), PWA admin + mobile remote control, HLS streaming (AAC segments +
  hls.js player, public segment serving), Slack/Discord on-air
  notifications (per-station webhooks).
- **Web framework**: migrated from Next.js 16 to a Vite + React SPA
  (TypeScript, Tailwind v4, shadcn/ui, react-router); static build served
  by nginx (Docker) or `web/serve.mjs` (bare metal).

### Fixed

- Web client uploaded to the wrong media endpoint (405) — library uploads
  were broken.
- Lua generator emitted `jingles({})` for stations without a jingles dir,
  which the engine rejected — station creation failed.
- Circular `--font-sans` CSS variable in the web app's Tailwind theme.

### Deployment

See `docs/release.md` for env vars, upgrade steps, and the verification
checklist.

[0.1.0]: https://github.com/sonyarianto/crabcast/releases/tag/v0.1.0
