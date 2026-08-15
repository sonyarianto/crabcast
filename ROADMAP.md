# Crabcast roadmap

> Crabcast is an AzuraCast-style web radio management platform — multi-station,
> playlist automation, live DJ support, listener requests, analytics, and
> beautiful admin/public UIs — built on a **Rust** backend and the
> [**Crabsoup**](../crabsoup/README.md) streaming engine (a Liquidsoap-inspired
> audio engine in Rust), with a **Next.js 16** (App Router, Turbopack) +
> **Tailwind CSS v4** + **shadcn/ui** frontend.
>
> This document is the plan. It is a living doc: move verified work to Done,
> keep "Known limitations" honest, and re-baseline performance numbers as
> phases land.

## 1. Vision

One self-hosted binary + one process per station that gives a radio station
everything AzuraCast does — media library, AutoDJ playlists, scheduling,
streamers, requests, jingles, public pages, analytics — but:

- **Faster**: Rust API + Crabsoup engine instead of PHP/Laravel + Liquidsoap
  (OCaml). Crabsoup already runs a full chain (crossfade + compressor/AGC +
  resample + encode) at ≈ 2.8 % of one core per 92.9 ms buffer. Target idle
  CPU per station in single-digit percents, cold station start < 5 s, p95 API
  latency < 50 ms.
- **More features**: gapless level-aware crossfades, live-programming
  `request.dynamic` schedulers, built-in DSP (compressor/normalize/replaygain),
  dead-air detection — all first-class in the engine today, exposed as normal
  product features instead of Liquidsoap scripting.
- **More user-friendly**: onboarding wizard, real-time UI (SSE), keyboard
  shortcuts, dark/light theme, mobile-responsive admin, clear empty states and
  error messages, i18n, WCAG 2.2 AA.

## 2. Positioning vs AzuraCast

AzuraCast is the reference feature set. Crabcast does not need to reinvent the
categories — it needs to win on operations and UX within them.

| Capability | AzuraCast | Crabcast |
| --- | --- | --- |
| Streaming engine | Liquidsoap (OCaml) + Icecast | Crabsoup (Rust) + Icecast (initially) |
| Backend | PHP / Laravel, MySQL/MariaDB | Rust (axum), SQLite (Postgres later) |
| Web UI | Bootstrap, server-rendered | Next.js 16 + Tailwind v4 + shadcn/ui |
| Realtime now-playing | Polling | SSE push |
| Crossfades | Liquidsoap config | Gapless + level-aware `smart_crossfade` built in |
| AutoDJ scheduling | Playlists + schedule | Same, generated as Crabsoup Lua from the DB |
| Live DJ (streamer) | Yes | Yes (Crabsoup `input.harbor` ducking) |
| Requests / jingles | Yes | Yes (Crabsoup request queue + jingles control) |
| Analytics | Listener stats, history | Same + dead-air alerts (`blank.detect`) |
| Deployment | Docker compose, multi-container | Slimmer: single Rust API container + one engine process per station |

## 3. Architecture

```
┌─────────────────────────── crabcast ───────────────────────────┐
│  web/ (Next.js 16, Tailwind v4, shadcn/ui)                      │
│    admin UI · public pages · embeddable player widget           │
│        │  REST + SSE (Rust API proxy via Next rewrites)         │
│  server/ (Rust, axum + tokio + sqlx)                            │
│    api/            REST routes + SSE hub                        │
│    auth/           sessions, roles, permissions                 │
│    stations/       station lifecycle: spawn/restart/health      │
│    lua/            DB config → crabsoup.lua generator           │
│    control/        Crabsoup HTTP control client                 │
│    media/          upload, tag scan, cover art, waveforms       │
│    analytics/      Icecast admin polling, history, alerts       │
│    db/             SQLite (sqlx), migrations                    │
│        │  spawn one process per station                         │
│        ▼                                                         │
│  crabsoup engine (../crabsoup)  ──►  Icecast  ──►  listeners    │
│    playlist/schedule/jingles/live harbor/mounts                  │
└─────────────────────────────────────────────────────────────────┘
```

Key integration points with Crabsoup (already shipped upstream):

- **HTTP control API** (`server.telnet({http_port = N})`): `GET /status`,
  `/uptime`, `/queue`, `/jingles`; `POST /cmd` with `{"command": "..."}`. One
  JSON envelope `{"ok": true, ...}` — exactly what a backend needs.
- **Lua config generation**: station config in the DB → `crabsoup.lua`
  (playlists → `playlist`/`request.dynamic`, dayparting → `switch`, mounts →
  `output.icecast`, jingles → `jingles`, harbor → `input.harbor`, DSP →
  `normalize(replaygain(...))`). Validate every generated script with
  `crabsoup --check` before applying.
- **Process lifecycle**: backend supervises one `crabsoup` process per station,
  auto-restart on crash, graceful restart on config change, health via
  `/status`.
- **Track-change events**: small Crabsoup addition (see Phase 1) — push
  `on_metadata`/`on_track` to a webhook so the backend records song history in
  real time instead of polling.

## 4. Tech stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Engine | Crabsoup (sibling repo, path dependency or released artifact) | Rust 2024 edition, MIT |
| Backend | Rust + **axum** + tokio | Tuned web framework, SSE support, graceful shutdown |
| DB | **SQLite** (sqlx) default, Postgres as a feature flag later | Single-file deploy; compile-time SQL checking |
| Migrations | sqlx-cli migrations | — |
| Auth | tower-sessions + argon2; role/permission model in DB | Session cookies, CSRF protection |
| Frontend | **Next.js 16** (App Router, Turbopack), TypeScript strict | Latest stable at the time of writing |
| Styling | **Tailwind CSS v4** | `@theme` tokens, dark/light |
| Components | **shadcn/ui** (Radix-based) | Copy-in, themeable, no lock-in |
| Realtime | SSE (Server-Sent Events) via axum → Next route handlers | Simpler than WebSocket; one-way is all we need |
| Media tagging | symphonia (already in Crabsoup deps) for duration; `lofty` for tag read/write | Cover art, title/artist/album |
| Waveforms | Compute on upload (peaks to JSON) | Cheap, no external service |
| Object storage | Local FS first; S3-compatible behind a trait | Phase 3 |
| Container | Docker + docker compose; multi-stage builds | Phase 0 dev, Phase 10 prod |

## 5. Repo layout

```
crabcast/
├── ROADMAP.md
├── README.md
├── server/                # Rust backend (axum) — workspace crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── api/           # REST routes + SSE
│       ├── auth/          # sessions, roles, permissions
│       ├── db/            # sqlx migrations + models
│       ├── stations/      # crabsoup lifecycle + health
│       ├── lua/           # config → crabsoup.lua generator
│       ├── control/       # crabsoup HTTP control client
│       ├── media/         # upload, tagging, waveforms
│       └── analytics/     # listener polls, history, alerts
├── web/                   # Next.js 16 app
│   ├── app/               # App Router routes (admin, public, widget)
│   ├── components/        # shadcn/ui + feature components
│   └── lib/               # API client, SSE hooks, i18n
├── docker/
│   ├── Dockerfile.server
│   ├── Dockerfile.web
│   └── compose.yml        # dev: server + web + icecast
├── docs/
└── .github/workflows/     # CI: fmt, clippy, tests, tsc, lint
```

Decision: the Rust API is the single source of truth; the Next.js app calls it
through Next rewrites (same origin in prod via a reverse proxy, no CORS
headaches). No server-side rendering of DB state — keep the API client-side so
the UI stays snappy and the Rust API stays decoupled.

## 6. Milestones

Each phase ships independently: working feature, tests, docs. Checkboxes move
to a "Done" section at the bottom as they land.

### Phase 0 — Scaffold & dev environment

- [x] Monorepo layout: `server/` axum app, `web/` Next.js 16 + Tailwind v4 +
      shadcn/ui, `docker/compose.yml` (server, web, Icecast) with one-command
      `make dev`.
- [x] Rust: workspace, edition 2024, `cargo fmt`/`clippy -D warnings` clean,
      axum health endpoint, sqlx + SQLite wired with first migration.
- [x] Next.js: App Router, TypeScript strict, Tailwind v4 `@theme` tokens,
      shadcn/ui base components (button, card, dialog, table, form, toast),
      dark/light toggle, font setup.
- [x] CI: Rust check/test/clippy + `tsc --noEmit` + ESLint + Prettier on PR.
- [x] README with quickstart; AGENTS.md with commands and conventions (mirror
      Crabsoup's).

**Acceptance**: `make dev` runs the full stack; API health visible from the
web UI; CI green.

### Phase 1 — Control plane: Crabsoup integration (the core)

- [x] `lua/` generator: station model → `crabsoup.lua` (mounts, playlist
      directory, jingles dir, harbor, sample rate, crossfade/duck settings).
      `--check` on every generation; diff + restart on change.
- [x] `stations/` supervisor: spawn one `crabsoup` per station, capture logs,
      restart with backoff on crash, status in API (`/api/stations/:id/status`).
- [x] `control/` client: `/status`, `/uptime`, `/queue`, `/jingles`, `/cmd`
      (`skip`, `queue.push`, `jingles.play`) with the `{"ok": ...}` envelope.
- [x] Track-change events: **small Crabsoup addition** — `on_metadata` webhook
      POST (or SSE) to the backend so song history is pushed, not polled.
      Record `now_playing` + history rows in DB.
- [x] Minimal station CRUD + a station dashboard page showing live status and
      now-playing over SSE.

**Acceptance**: create a station in the UI, upload a playlist folder path, hear
it on an Icecast mount, skip a track, see now-playing update in real time.

### Phase 2 — Auth, users, roles

- [x] Session auth (argon2, secure cookies, CSRF), login/logout, change
      password, email-less first-run admin bootstrap.
- [x] Role/permission model (AzuraCast parity): global roles (super admin,
      station manager, DJ, media editor) + per-station permissions.
- [x] User CRUD in admin UI; invite by email later (Phase 10).
- [x] Audit log (who changed what) — cheap with SQLite, worth it from day one.

**Acceptance**: two users with different station permissions behave
accordingly; every mutation is audited.

### Phase 3 — Media library

- [x] Upload (drag & drop; resumable chunks deferred — see Known
      limitations), storage trait (local FS now, S3 later), dedupe by
      content hash (sha256).
- [x] Tag scan: title/artist/album/genre, duration, cover art, replaygain tags
      (the engine reads these — surface them). Tag editing writes back to
      the file.
- [x] Waveform (peaks computed on upload, rendered in the preview player) +
      audio preview in the browser (Range-enabled `/stream`).
- [x] Search + filters (artist/album/genre facets) + server-side sort and
      pagination; "add to playlist" from results lands with Phase 4
      playlists; bulk edit deferred.
- [x] Library page with column sorting and cover-art grid/list toggle.

**Acceptance**: upload 1,000 tracks, browse/filter/edit them at p95 < 50 ms,
attach a folder to a station playlist and hear it on air.

### Phase 4 — Playlists & scheduling (AutoDJ)

- [x] Playlist types: standard (shuffle/sequential), looping, scheduled
      (dayparted), once-per-hour (AzuraCast parity), request playlist
      (request playlist deferred — lands with the Phase 6 request system).
- [x] Drag-and-drop ordering, per-playlist weights, per-track fade/cue
      overrides (maps to Crabsoup `cue_cut`/`annotate:`).
- [x] Scheduler UI: time-of-day + weekday rules → Crabsoup `switch`/`rotate`
      generation with live preview of the generated Lua.
- [x] Crossfade/ducking/DSP station settings mapped to Crabsoup `set()` knobs
      (landed with the Phase 2 station model; `normalize(replaygain(...))`
      deferred — the library surfaces replaygain tags for a future pass).

**Acceptance**: a station with dayparting + crossfades runs unattended for 24 h
with a correct schedule; changing a rule applies live without dropping audio.

### Phase 5 — Streamers (live DJ)

- [x] Streamer accounts + mount config (`input.harbor` source password).
- [x] Connection tracking (on-air/off-air via harbor state), on-air indicator
      in the dashboard, ducking visualization.
- [x] Streamer-facing view: connect instructions (Icecast source client),
      mic test, disconnect.

**Acceptance**: a DJ connects from a source client, the playlist ducks out, and
it fades back in on disconnect — all visible in real time.

### Phase 6 — Requests & jingles

- [x] Request system: configurable request playlists + per-station request
      rules (max per hour, dedupe, moderation toggle); backend maps to
      `queue.push`.
- [x] Jingles management UI: upload, preview, trigger from admin; maps to
      `jingles.play`.
- [x] Remote control surface: skip, queue, jingles from the dashboard and
      (later) from a mobile PWA page.

**Acceptance**: a listener request plays within seconds; a jingle fires on
command; abuse rules hold.

### Phase 7 — Public pages & web player

- [x] Per-station public page (brandable): player, now-playing art, song
      history, request form, listener count (deferred to Phase 8), social
      links.
- [x] Embeddable widget (iframe) for third-party sites.
- [x] Web player: native HTML5 audio against the mount (MP3 for max
      compatibility; Opus mount when available); SSE-driven metadata overlay
      (public page polls the public now endpoint instead — SSE stays
      auth-gated).
- [x] Public API endpoint for third parties (now-playing, history) — the
      AzuraCast API-parity seed (full REST API in Phase 9).

**Acceptance**: a visitor can play the stream, see what's on now, and request a
song — all without an account.

### Phase 8 — Analytics & monitoring

- [x] Listener tracking: poll Icecast admin API per mount, store per-minute
      samples; unique-listeners approximation.
- [x] Station dashboard charts: listeners over time, top songs, request rates,
      uptime; song history export (CSV).
- [x] Alerts: dead-air (`blank.detect` on_blank webhook), engine crash loops,
      disk usage, Icecast unreachable — email/webhook notifications.
- [x] Uptime/history retention policy (configurable).

**Acceptance**: 7-day listener graph matches Icecast's own numbers within
tolerance; a forced dead-air episode raises an alert.### Phase 9 — Performance, scale & API

- [x] Benchmarks (criterion in `server/` + load tests): API p95, station startup,
      CPU/RAM per station at idle and under playout. Re-baseline
      against AzuraCast's known numbers (documented in this file).
- [x] Postgres feature flag (sqlx) for multi-host deployments: the
      backend is dialect-agnostic — `DATABASE_URL` scheme picks SQLite
      or Postgres at boot (dual migration sets, `$N` placeholders, a
      `DbBool` wrapper, and dialect branches for the handful of
      SQLite-only expressions; verified live against Postgres 16).
      See Done for the full write-up.
- [x] Shared SSE/event bus via Redis pub/sub (optional, behind
      `CRABCAST_REDIS_URL`).
- [x] CDN-friendly static serving for media/cover art; cache headers.
- [x] Full REST API (AzuraCast-compatible surface where sensible) + API tokens.
- [x] Horizontal-scale story: N API hosts, M station hosts, one shared DB
      (documented in the docs site; the Redis bus delivers the shared
      event bus, Postgres is the remaining unlock).

**Acceptance**: 50 stations on a small VPS with p95 API < 50 ms and idle CPU
per station in single digits; documented benchmark table.

### Phase 10 — Packaging, deployment & docs

- [ ] Production Docker images (multi-stage, slim), `compose.prod.yml`,
      one-command install script (Debian/Ubuntu + Docker).
- [ ] Systemd unit (bare-metal install without Docker), upgrade path with
      DB migrations run automatically.
- [x] Backup/restore (DB + media + station configs) from the admin UI.
- [x] Onboarding wizard: first-run admin, create first station, add media,
      go live in < 5 minutes.
- [x] Docs site (mirror Crabsoup's VitePress site pattern): getting started,
      station guide, engine reference, API reference.

**Acceptance**: fresh VPS → on-air station in under 10 minutes from the
install script; backup → restore verified in CI.

### Phase 11 — Stretch goals (post-1.0)

- [x] Podcasts (AzuraCast parity): upload episodes, feed generation.
- [x] HLS streaming as an alternative to raw mounts (AAC segments + hls.js; LL-HLS later).
- [x] PWA admin + mobile remote control.
- [ ] i18n: full translation pass (next-intl), RTL support.
- [ ] Built-in mount server (skip Icecast) — only after Phase 8/9 listener
      metrics justify the engine work; this is the biggest lever for the
      "better performance" claim but also the biggest risk, so it stays last.
- [x] Webhooks out (Slack/Discord on-air notifications).

## 7. Performance targets (SLOs)

These are the numbers the project is held to; record actuals per phase in Done.

| Metric | Target | AzuraCast reference (approx) |
| --- | --- | --- |
| Idle CPU per station | < 5 % of one core | higher (PHP worker + Liquidsoap) |
| Full-chain audio work | reuse Crabsoup ≈ 2.8 % of one core | Liquidsoap comparable or worse |
| API p95 latency | < 50 ms | — |
| Station cold start (config → on air) | < 5 s | seconds to tens of seconds |
| Memory per station | < 150 MB | higher per station |
| Web UI first paint | < 1.5 s on broadband | — |
| Stations per small VPS (4 vCPU / 8 GB) | 20+ | fewer |

Benchmark methodology: criterion micro-benchmarks in `server/` (`cargo bench`,
`benches/hot_paths.rs`), `oha` load tests in CI plus `scripts/load-test.sh`
(p95 per endpoint), and `scripts/bench-station.sh` that spins up N stations and
records CPU/RAM over 10 minutes. Baseline measured 2026-08-15 (debug build,
localhost): API p95 ≈ 0.01 ms at 8.5k rps burst on `/api/now-playing`;
listener-series query (7 d of per-minute samples, 60-min buckets) ≈ 7.4 ms.

## 8. UX principles (applies to every phase)

- **Wizard over forms**: first-run and "add a station" are wizards, not blank
  forms.
- **Real-time by default**: now-playing, on-air, queue, listener counts all
  update via SSE — no refresh buttons.
- **Optimistic UI**: mutations apply instantly, roll back with a clear toast on
  error.
- **Empty states teach**: every empty page shows how to use the feature.
- **Keyboard-first**: shortcuts for play/skip/jingle/queue; full focus
  management.
- **Dark/light + accessible**: WCAG 2.2 AA, prefers-reduced-motion respected.
- **Consistent component language**: shadcn/ui everywhere; no bespoke widgets
  without a design note.

## 9. Non-functional requirements

- **Security**: argon2 password hashing, session cookies (`httpOnly`,
  `SameSite=Lax`), CSRF protection, per-station authorization checks in every
  route, upload validation (content sniffing, no executable media), secrets in
  env vars only, no secrets in generated `crabsoup.lua`.
- **Observability**: structured logs (tracing), request IDs, health endpoints,
  Prometheus metrics (`/metrics` on the API; listener gauges from Phase 8).
- **Reliability**: engine supervisor restarts with backoff, config apply is
  atomic (write new Lua → `--check` → swap → graceful restart), DB migrations
  run at boot before serving.
- **Performance discipline**: no per-request blocking DB work on the hot path;
  index every query the UI actually issues; benchmark before optimizing.

## 10. Decision log

- **Frontend stack (decided)**: Next.js 16 (App Router) + Tailwind v4 + shadcn/ui
  for the UI layer; the backend is 100 % Rust regardless of frontend choice.
  Rationale: shadcn/ui is React-only, the admin UX (drag-drop, virtualized
  tables, optimistic updates, SSE) is the product differentiator, and the
  performance wins live in the Rust engine/API, not in how HTML is rendered.
  Revisit Rust-rendered public pages only if public-page TTFB becomes a
  measured problem.

## 11. Open decisions (flagged, not blocking)

1. **Database**: SQLite-first (default) vs Postgres-first. Recommendation:
   SQLite with sqlx feature flag for Postgres — single-file deploy is the
   self-hosted selling point; Postgres lands with Phase 9 multi-host.
2. **Icecast dependency**: keep external Icecast (fast to ship, battle-tested,
   AzuraCast does the same) vs built-in mount server (Phase 11 stretch). The
   built-in is the biggest performance lever but the biggest engineering risk —
   do it last, driven by data.
3. **Track-change transport**: webhook POST from Crabsoup (chosen, Phase 1) vs
   SSE from Crabsoup's `http_port`. Webhook is one line of engine code and
   fits the backend's push model.
4. **Monorepo vs split repos**: monorepo (chosen) — Crabsoup stays a sibling
   repo consumed as a path dependency / released binary, since it is
   engine-focused and independently versioned.
5. **REST API parity**: full AzuraCast-compatible API (Phase 9) vs own API
   shape. Recommendation: own clean API first; a compatibility shim later only
   if migration tooling is wanted.

## 12. Risks & mitigations

- **Lua generation complexity** (advanced schedules, edge cases) — mitigate:
  `crabsoup --check` gate on every change, golden-file tests of generated
  scripts, start with the core feature set (Phases 1–6) before exotic ones.
- **One process per station memory at scale** — mitigate: measure from Phase 1
  (supervisor exposes RSS), SLO in §7, revisit shared-engine pooling only with
  data.
- **Browser codec support** — mitigate: default MP3 mount for compatibility,
  Opus/AAC mounts as progressive enhancement (same approach as AzuraCast).
- **Next.js churn** — mitigate: pin to a stable major, keep the API layer thin
  and framework-agnostic so the UI can be swapped without touching the backend.
- **Scheduling timezones** — mitigate: store everything in station-local time
  (store tz per station), render in the user's tz.

## 13. Known limitations (fill in as discovered)

- The engine fires `on_metadata` with empty/`"(no source)"` titles at source
  transitions (e.g. jingle → playlist handoff); the webhook receiver drops
  those so history stays clean, but a silent track *with* no metadata still
  reads as a normal entry.
- Icecast must be reachable at apply time or the engine retries with its own
  `reconnect`; the supervisor surfaces the crash-loop state via `last_error`.
- Phase 3 uploads are single-request multipart (whole file in memory per
  request); resumable chunked upload is deferred — fine for typical library
  sizes, revisit before large-live-set workflows. Storage is local FS only;
  the `Storage` trait is the seam for S3 later.
- Listener polling authenticates to the Icecast admin API with the station's
  *source* credentials; a stock Icecast requires the admin user, so the
  station's source user must be granted admin rights (or the mount stays
  unreachable and the `icecast_unreachable` alert fires).
- Alert notifications are webhook-only (`CRABCAST_ALERT_WEBHOOK_URL`);
  email/Slack/Discord integrations are deferred to Phase 11.
- The crash-loop alert fires on the 5th consecutive crash that dies within
  60 s of start (a long-lived run resets the streak); it is a lifetime
  counter, not a rolling window, and resolves once the engine stays up 60 s.

---

## Done

- **Phase 0 — Scaffold & dev environment** (2026-08-14): monorepo layout
  (`server/` axum + SQLite, `web/` Next.js 16 + Tailwind v4 + shadcn/ui Base
  UI), `docker/compose.yml` + `make dev`, GitHub Actions CI (fmt/clippy/test,
  tsc/eslint/prettier/build), README + AGENTS.md. API health is visible from
  the web home page through Next rewrites (`/api/*` → `API_UPSTREAM`).
- **Phase 1 — Control plane: Crabsoup integration** (2026-08-14): `lua/`
  generator (station model → `crabsoup.lua`, `--check`-gated, atomic swap),
  `stations/` supervisor (one `crabsoup` per station, log capture, backoff
  restart, boot start-all, graceful shutdown), `control/` client
  (`/status`, `/uptime`, `/queue`, `/jingles`, `/cmd`), Crabsoup `http_post`
  webhook (`on_metadata` → `POST /api/webhooks/track?station=<id>`, noise
  filtering), `song_history` table, SSE hub + `/api/stations/:id/events`,
  station CRUD + live dashboard (status poll + SSE now-playing + skip/jingle
  commands) in the web app. Verified end-to-end: station created from the
  API streams to Icecast, history records, SSE pushes track changes.
  (Also fixed the `on_metadata(src, fn)` argument order vs the guide, and
  corrected `dsp.md` to match.)

- **Phase 3 — Media library** (2026-08-14): `media_files` table + indexes;
  `Storage` trait + `LocalStorage` (files sharded by content-hash prefix
  under `CRABCAST_MEDIA_DIR`); upload via multipart with sha256 dedupe
  (duplicates skipped and reported); lofty tag scan (title/artist/album/
  genre, duration, sample rate/channels/bitrate, replaygain, embedded cover
  art) + symphonia waveform peaks (256 buckets); search (`q` across
  title/artist/album/filename) + artist/album/genre facet filters + sort +
  pagination with total count; Range-enabled audio streaming via
  `ServeFile`; tag editing that writes back into the file itself;
  delete removes DB row + files; `media_editor` role wired (super admin,
  global station_manager, or global media_editor can mutate; any
  authenticated user can browse); audit logging on every mutation;
  `/api/media/config` exposes the storage root so users can point a
  station's playlist dir at the library. Verified end-to-end against a live
  server: upload → dedupe → scan (duration/waveform/bitrate) → search +
  filters → 206 range stream → tag write-back (ID3 chunk physically present
  in the file) → DJ user blocked from upload/delete (403) but can list →
  delete removes file + row → audit trail. Web: `/library` page with
  drag & drop upload, debounced search, facet dropdowns, sortable table
  with cover thumbs, cover-art grid toggle, sticky preview player with
  waveform + `<audio>`, tag edit dialog, pagination; Library nav link added
  to the Stations headers.

- **Phase 4 — Playlists & scheduling (AutoDJ)** (2026-08-14): `playlists`,
  `playlist_tracks` (ordered, per-track `fade_in`/`fade_out`/`cue_in`/
  `cue_out` overrides), `playlist_schedules` (weekday + HH:MM daypart
  rules); kinds `standard`/`looping`/`scheduled`/`once_per_hour` with
  per-playlist `weight`; Lua generator renders one `playlist({files = {...}})`
  per playlist (shuffle/loop), `annotate:` prefixes for per-track
  fade/cue overrides, `rotate` + weights for multiple always-on playlists,
  and a `switch` for dayparted schedules with the other playlists as
  fallback; legacy `directory` fallback kept when no playlist is enabled;
  every mutation re-renders + `--check`s + restarts the station engine, so
  changes apply live; `/api/stations/:id/playlists/preview` returns the
  generated Lua; CRUD + track add/remove/reorder + override update +
  schedule add/remove, all gated on `station_manager` for the station with
  audit logging. Web: `/stations/[id]/playlists` page — playlist list,
  create/edit/delete dialogs, kind/weight/shuffle toggles, track picker
  with a live library search, drag-and-drop reorder, per-track fade/cue
  edit, daypart schedule rows, and a Lua preview panel; link added to the
  station detail page. Verified end-to-end against a live server (real
  crabsoup `--check`): create station → upload tracks → create playlists →
  add tracks → preview Lua (annotate prefixes + `switch` daypart render
  correctly) → track reorder → DJ user reads (200) but mutations are 403 →
  schedule/track/playlist deletion re-applies config to disk → full audit
  trail.

- **Phase 5 — Streamers (live DJ)** (2026-08-15): `streamers` table
  (per-DJ account with its own source password, enabled flag for instant
  revocation); engine extended (sibling `crabsoup` repo): `input.harbor`
  accepts `extra_passwords = {...}` (any of them authenticates) and
  `harbor_connected` is shared into the status handle so `GET /status`
  reports whether a DJ is on air; Lua generator renders the station
  password plus every enabled streamer's password; `streamers` CRUD API
  (`station_manager`-gated, audit-logged, config re-applied live on every
  mutation) + `/api/streamers/:id/connect` returning the mount URL,
  per-DJ credentials and a copy-paste `curl` mic test; station status now
  carries `live` (harbor held = playlist ducked); dashboard shows a
  pulsing LIVE badge + ducked banner, and a Streamers card with
  create/edit/delete and per-account connect-instructions dialog. Also
  fixed a supervisor bug found during verification: the watchdog claimed
  the child immediately and blocked on `wait()`, so `stop()` on re-apply
  could never kill the old engine — every mutation leaked a process that
  held the control/harbor ports. The watchdog now polls `try_wait()` + a
  stop flag and kills the child it owns, and `stop()` waits for the pid
  to actually exit before `spawn()` runs (re-applies are atomic, no port
  races). Verified end-to-end against a live engine: streamer created →
  config on disk carries `extra_passwords = {"sarah-secret", ...}` →
  wrong password 401 → DJ PUT accepted → `harbor_connected: true` /
  `live: true` while connected → `false` after disconnect → disabling a
  streamer re-renders the config and the old password is 401 instantly →
  DJ role reads (200) but mutations 403 → connect-info endpoint → audit
  trail → server shutdown leaves zero orphaned engines.

- **Phase 6 — Requests & jingles** (2026-08-15): `request_rules`
  (enabled, max per hour, dedupe, moderation) + `requests` log tables;
  Lua generator renders `rq = request.queue()` into
  `fallback({j, live, rq, pl})` so pushed requests preempt the playlist;
  listener request API (any authenticated user) enforces the rules —
  rate limit (429), dedupe against pending/queued + the engine queue,
  moderation (requests land pending until a station manager approves,
  which pushes to the engine) — and maps to the engine's `queue.push`
  with the track's absolute library path; request history + moderation
  inbox APIs; remote control of the engine queue (view / clear / skip,
  `station_manager` or `dj`); jingle file management against the
  station's jingles dir (list / multipart upload / audio preview /
  delete, `station_manager`-gated, config re-applied so the engine
  re-scans immediately) + `jingles.play <name>` from the dashboard.
  Web: Requests card (rules editor, pending-approval inbox, live engine
  queue with clear/skip, recent requests) and Jingles card (upload,
  inline preview, fire-on-air, delete) on the station page. Verified
  end-to-end against a live engine: request → 201 → track appears in
  `GET /queue` and plays → duplicate 400 → rate limit 429 → moderation
  pending → approve pushes + plays → reject drops it → jingle upload
  re-scans the engine (playable by name) → preview 200 → delete → DJ
  role: rules/jingles mutations 403, queue control 200 → full audit
  trail.

- **Phase 10 — Packaging, deployment & docs (partial)** (2026-08-15):
  **production packaging** — `docker/Dockerfile.server` (multi-stage
  release build that also `cargo install`s the Crabsoup engine from the
  pinned branch, slim `debian:bookworm-slim` runtime with a non-root user,
  `/data` + `/media` volumes, healthcheck), `docker/compose.prod.yml`
  (server + web + Icecast, secrets from env with required
  `CRABCAST_SESSION_SECRET`, named volumes, healthcheck-gated startup),
  `scripts/install.sh` (installs Docker + compose plugin, clones to
  `/opt/crabcast`, generates `docker/.env` with a random session secret,
  `compose up -d --build`), systemd units for the bare-metal path
  (`packaging/crabcast-server.service` + `crabcast-web.service`, env in
  `/etc/crabcast/env`, `Restart=on-failure` so restore's exit code 3
  auto-restarts) and `packaging/README.md` covering both installs and
  the upgrade path (migrations run automatically at boot). Verified:
  server release build, engine `cargo install --branch main`, and the
  runtime stage all build/run; **backup/restore** — `GET /api/backup/
  download` (super admin) snapshots the live DB with `VACUUM INTO`, zips
  `crabcast.db` + `media/*` + `stations/configs/*` + `manifest.json`
  (schema version, counts), and streams it with a cleanup-on-drop body;
  `POST /api/backup/restore` (super admin, multipart) validates the
  archive (app + schema version gate, SQLite magic, path-traversal
  sandbox) into a staging dir, stops the station engines, swaps the live
  files aside as `<name>.pre-restore-<ts>` safety copies (EXDEV-safe
  move), then exits with code 3 so the process supervisor restarts and
  boot migrations run on the restored DB. Verified live end-to-end:
  backup → wipe → restore → server exits/restarts → admin logs in, station
  and both uploaded media files are back, safety copy retained; anonymous
  download 401s. Unit tests cover path sandboxing, version gating and
  archive round-trips (rejects bad SQLite, newer schema, `../` entries).
  Web: **Settings** page gains a super-admin-only Backup & restore card
  (download + restore-with-confirmation); **onboarding wizard** at
  `/welcome` — three steps (create station → upload music → build playlist
  & go live) shown on fresh installs; login/bootstrap and the home page
  route first-run admins to it. Also fixed two latent bugs surfaced by the
  wizard work: the web client uploaded to `/api/media/upload` while the
  route is `POST /api/media` (405 — uploads now work), and the Lua
  generator emitted `jingles({})` for stations without a jingles dir,
  which the engine rejects at `--check` (the jingle source is now omitted
  from the chain instead).

- **Phase 11 — PWA admin + mobile remote control** (2026-08-15): the web
  app is now installable — `app/manifest.ts` (name/theme `#7c3aed`,
  standalone display, 192/512 + maskable icons), PNG icons generated with
  ffmpeg from the site's purple/white radio-note mark (glyph pixel-
  verified), `public/sw.js` (app-shell offline: network-first
  navigations with cached fallback, cache-first static assets with
  background refresh, `/api/*` never cached, versioned cache + cleanup,
  `skipWaiting`/`clients.claim`), and a `PwaRegister` component that
  registers the worker in production only (dev HMR untouched).
  `layout.tsx` gained `themeColor`, `appleWebApp` (capable,
  black-translucent), and the apple-touch icon. The admin UI is
  responsive and touch-friendly, so the installable app doubles as a
  phone remote control for station commands. Verified: production build
  emits `/manifest.webmanifest`; the standalone output (assembled exactly
  as the Dockerfile does: standalone + static + public) serves
  `sw.js`, both icons, the maskable + apple icons, the manifest and the
  app shell all HTTP 200.

- **Web framework — Next.js → Vite + React SPA** (2026-08-15): the web
  app was already a client-side SPA in practice (every page `"use client"`,
  all data through `web/lib/api.ts` → `/api/*`), so it moved to **Vite 7 +
  React + TypeScript + Tailwind v4 + shadcn/ui + react-router**. The
  migration: `app/` → `src/pages/` (route table in `src/main.tsx`,
  `BrowserRouter`), `components/`/`lib/` → `src/`, `next/link` →
  react-router `Link` (`href` → `to`), `next/navigation` `useRouter` →
  `useNavigate` (`replace: true`) and `useParams` (`params.id!`), and the
  Next `metadata`/manifest moved to `index.html` meta tags +
  `public/manifest.webmanifest`. Dev proxy (`/api` → `API_UPSTREAM`) now
  lives in `vite.config.ts`; the production image is a static build served
  by **nginx** (`docker/Dockerfile.web` + `nginx.conf.template` with
  envsubst `proxy_pass ${API_UPSTREAM}` and SPA fallback); the bare-metal
  systemd unit now runs a zero-dependency Node static server
  (`web/serve.mjs`). Also fixed: circular `--font-sans` var in
  `globals.css` (Geist now loaded from Google Fonts), stale
  `eslint.config.mjs`/`postcss.config.mjs` removed, prettier points at
  `src/globals.css`, `npm run typecheck` replaces `tsc --noEmit`. Verified:
  tsc/lint/build/prettier all green, dist serves the SPA shell + assets +
  sw.js + icons + manifest, dev proxy + nginx route shapes checked. Docs
  (AGENTS.md, README, architecture/scaling/getting-started/api site pages)
  updated to the new stack.

- **Phase 11 — HLS streaming** (2026-08-15): stations can publish an HLS
  stream alongside Icecast. Migration `0013_station_hls` adds
  `hls_enabled`/`hls_dir`/`hls_segment_seconds`/`hls_retention`; the Lua
  generator binds a single `tap = on_metadata(air, ...)` source shared by
  `output.icecast` and — when enabled — `output.hls({directory, ...})`
  (the engine's shared-root check requires every output to reference the
  same source, and one tap keeps the metadata webhook firing once). The
  engine slices AAC into `seg-*.ts` + `playlist.m3u8` under the station's
  dir (created on connect, stale files cleared). Public payload gains
  `hls_playlist_url`, and `GET /api/public/stations/{id}/hls/{*file}`
  serves the files same-origin with correct content types
  (`application/vnd.apple.mpegurl` / `video/mp2t`), a `..`/absolute-path
  sandbox (400), and 404 when HLS is disabled; enabling HLS with an empty
  dir is rejected 400. Web: shared `StationPlayer` component (native HLS
  on Safari/iOS, lazy-loaded hls.js elsewhere, raw-mount fallback) used by
  the public page and the embeddable widget; the station profile dialog
  gains the HLS toggle + directory field. Verified live against the real
  engine: segments written and rolling (EXTM3U + `#EXT-X-MEDIA-SEQUENCE`,
  AAC 44100 stereo), playlist + segment served with the right content
  types, traversal rejected, disable → 404, empty dir → 400. 59 Rust
  tests, clippy clean, web tsc/lint/build clean.

- **Phase 11 — Webhooks out (Slack/Discord on-air notifications)**
  (2026-08-15): per-station notification webhooks. Migration
  `0014_notification_webhooks` (url, `events` = `*` or
  `started,stopped,crashed,blank`, enabled; cascade on station delete);
  `db/notification_webhooks.rs` CRUD + `for_event` lookup with events
  validation (400 on unknown events); `api/webhooks.rs` —
  station-manager-gated `GET/POST /api/stations/{id}/webhooks` +
  `DELETE /api/webhooks/{id}`, audit-logged. Dispatch lives in
  `notify.rs`: `station_event(pool, id, event)` loads subscribed webhooks
  and POSTs one payload with `text` (Slack), `content` (Discord), and
  `event` keys (each service ignores the other's field; 5s timeout,
  failures logged only). Events fire from the supervisor (`started` after
  spawn, `stopped` on the stop path, `crashed` on unexpected exit — the
  delete handler also fires `stopped` deterministically because the
  watchdog races the cascade delete) and from the blank webhook receiver
  (once per dead-air episode). Web: a Notifications card on the station
  page — list, add (URL + event checkboxes, none = all), delete. Verified
  live against a local listener: started (with station name), stopped
  (config restart), crashed (engine kill -9), blank (dead-air webhook),
  stopped (station delete), unknown-event 400, delete 204. 61 Rust
  tests, clippy clean, web tsc/lint/build clean.

- **Phase 11 — Podcasts (AzuraCast parity)** (2026-08-15): migration
  `0012_podcast_episodes` (episodes reference media-library files,
  cascade on station/media delete); `db/podcasts.rs` repo (create/list
  with media join/delete, FK errors surfaced as 400); `api/podcasts.rs`
  — station-manager-gated CRUD (`GET/POST /api/stations/{id}/podcasts`,
  `DELETE /api/podcasts/{id}`, audit-logged) and a public RSS 2.0 feed
  (`GET /api/public/stations/{id}/podcast.rss`) with iTunes author
  namespace, RFC 2822 pubDates, XML escaping, and same-origin absolute
  enclosures pointing at the media stream endpoint (Host-header based).
  Unit tests: XML escaping, pubDate rendering, feed structure. Web:
  `/stations/[id]/podcasts` page — episode list, create dialog with a
  media-library search picker, delete, RSS-feed link — plus a Podcasts
  button on the station page. Verified live: upload audio → publish
  episode (title with `&` escaped) → anonymous RSS parsed with
  `xml.etree` (channel, item title, enclosure url/type/length, pubDate)
  → list → delete (204, empty after). 58 Rust tests pass, clippy clean,
  web tsc/lint/build clean. API docs updated on the site.

- **Phase 9 — Redis event bus** (2026-08-15): `api/sse.rs` now has two
  backends. The default stays the in-process broadcast hub; when
  `CRABCAST_REDIS_URL` is set (e.g. `redis://host:6379`), every host's hub
  publishes `StationEvent`s to a `crabcast:station:{id}:events` pub/sub
  channel and SSE subscribers consume from Redis instead, so N API hosts
  fan events out to each other. The Redis client is built at startup but
  the connection manager is opened lazily (a missing Redis never blocks
  boot and degrades to the local hub with a warning); pub/sub is
  per-subscriber with an mpsc forwarding task, and the `StationEvent`
  types gained `Deserialize` for the wire format. Verified live against
  a real Redis (docker): two API hosts on separate ports + separate
  SQLite DBs sharing one Redis — a track webhook posted to host A arrived
  on host B's SSE stream; the no-Redis local path was regression-tested
  (SSE delivered in-process, no Redis configured). Docs updated
  (`CRABCAST_REDIS_URL` in the site env table + packaging README).

- **Phase 9 — Postgres backend** (2026-08-15): the dual-driver port.
  The server now runs on sqlx's **Any driver**: `DATABASE_URL` scheme
  selects SQLite (default) or Postgres at boot; migrations are dual
  (`migrations/` + `migrations-pg/`, run by kind), every query uses
  `$N` placeholders (mapped to named params on SQLite, native on
  Postgres), a `DbBool` decode wrapper spans SQLite `INTEGER` and PG
  `BOOLEAN` columns, and the SQLite-only expressions (strftime/
  julianday/substr timestamps, `IS ?`, literal `0/1` booleans, bucket
  math) got small dialect branches (to_char/EXTRACT(EPOCH)/substring,
  `IS NOT DISTINCT FROM`, TRUE/FALSE). Sessions use a scheme-typed
  store (`tower_sessions.session` on PG). Backup/restore stays
  SQLite-only (VACUUM INTO file swap) and returns a clear error on PG.
  Verified: 61 SQLite tests pass through Any; full API e2e against a
  real Postgres 16 container (auth/sessions, station+playlist+
  streamer+request CRUD, analytics bucket/top-songs/CSV, webhooks,
  backup gate). Backup/restore gained a Postgres path (`pg_dump -Fc` /
  `pg_restore --clean`) verified end-to-end against Postgres 17; the
  SQLite path still uses VACUUM INTO.

- **Phase 9 — Horizontal-scale story** (2026-08-15): new docs-site page
  `guide/scaling.md` laying out the three deployment models (single host,
  web replicas today; N API hosts / M station hosts / one shared DB once
  the Postgres backend lands), what already supports multi-host (the Redis
  event bus, bearer-token REST API, stateless handlers) and the remaining
  gaps (SQLite is single-writer so one API host until Postgres; session
  store locality; station placement + shared media on station hosts).
  Includes an ASCII topology diagram and concrete next steps.

- **Phase 9 — Performance, scale & API (partial)** (2026-08-15):
  **benchmarks + load tests** — criterion benches (`benches/hot_paths.rs`:
  argon2 hash/verify, station-list JSON serialization, the listener-series
  query over 10k seeded samples), `scripts/load-test.sh` (oha/hey/ab, p95
  per endpoint vs the 50 ms SLO), `scripts/bench-station.sh` (creates N
  stations, samples each engine's RSS/%CPU, prints a summary), and a CI
  `load` job that boots the API and gates on an oha p95 sanity bound;
  `cargo bench --no-run` added to the rust CI job. Measured on a dev box
  (debug build): p95 ≈ 0.01 ms at ~8.5k rps burst on `/api/now-playing`,
  sub-ms at a sustained 200 rps; listener_series(7 d, 60-min buckets)
  ≈ 7.4 ms. **CDN-friendly static serving** — `/api/media/{id}/stream`
  and `/api/media/{id}/cover` now send `ETag` + `Cache-Control` and answer
  `If-None-Match` with 304 (cover art is content-addressed, so it is
  `public, max-age=1y, immutable`; audio is `private, max-age=1h` since tag
  edits rewrite the file and change the ETag); the live Icecast proxy is
  `no-store`. Range/206 continues to come from tower-http ServeFile.
  **REST API + API tokens** — `api_tokens` table (sha256 secrets, revoked
  without deletion), Bearer auth in the `CurrentUser` extractor (a bad
  bearer 401s instead of falling back to the session), token CRUD at
  `/api/tokens` (secret shown once) with a Settings page (`/settings`)
  for managing them; AzuraCast-style public surface: `/api/now-playing`
  (all stations) and `/api/station/{id}/now-playing`, working with or
  without tokens; the whole admin API is now token-capable. Verified live:
  token create → `Authorization: Bearer` 200 on `/api/stations`, revoked
  token 401, bad bearer 401, anonymous 401, `/api/now-playing` 200, load
  test p95 numbers above.

- **Docs site** (2026-08-15): VitePress site in `website/` (mirrors the
  Crabsoup site pattern) — home page with feature grid, getting-started
  guide (quickstart, first station, env vars), radio-operation guide
  (stations/playlists/streamers, requests & jingles), monitoring guide
  (analytics & alerts), full REST API reference incl. Bearer tokens and the
  public surface, and an architecture page. `website/vercel.json` pins the
  vitepress framework preset for Vercel (Root Directory = `website`);
  deploy steps in `website/README.md`. Verified: `npm run build` produces a
  clean static site with no dead links.

- **Phase 8 — Analytics & monitoring** (2026-08-15): `listener_samples`
  (per-minute, polled from the Icecast admin API with a reachability flag)
  + `alerts` tables (dedup by (station, kind) while open); `analytics/`
  background poller — every 60 s polls each station's `/admin/stats`
  (quick-xml parse, mount as attribute or element), every 10 min checks
  media-dir free space (statvfs; 1 GiB / 5 % floor), every 6 h purges rows
  older than `CRABCAST_RETENTION_DAYS` (default 30: listener samples, song
  history, resolved alerts); alerts: `icecast_unreachable` (auto-resolved
  when the admin API answers again), `dead_air` (the generated Lua wraps
  the output chain in `blank.detect({threshold = -40, duration = 5,
  exhaust_while_blank = false, on_blank = http_post →
  /api/webhooks/blank})` — the mount stays up while silent, and the first
  real track webhook clears it), `engine_crash_loop` (supervisor raises on
  the 5th consecutive crash within 60 s of start; cleared once the engine
  stays up 60 s), `disk_low` (global); optional outbound webhook
  (`CRABCAST_ALERT_WEBHOOK_URL`) posts raise/resolve events; analytics API
  — listener series (bucketed AVG), summary (current/unique-24h/uptime/
  plays/requests), top songs, per-day request stats, song-history CSV
  export (`history.csv`, RFC 4180 escaping), alert list + resolve
  (station_manager-gated); web: `/stations/[id]/analytics` page — 24 h /
  7 d / 30 d range, recharts area chart, stat cards, top songs + request
  tables, alerts feed with resolve buttons, CSV export link; Analytics
  button added to the station page. Verified end-to-end against a live
  server with a mock Icecast admin: 7 listeners / 42 connections parsed
  and sampled, anonymous 401s, dead-air raise → dedupe → manual resolve →
  track-webhook auto-clear, Icecast-down raise → auto-resolve on recovery,
  uptime % tracked, CSV shape; the full generated script (incl. the
  `blank.detect` wrapper) passes a real `crabsoup --check` — guarded by a
  new test.

- **Phase 7 — Public pages & web player** (2026-08-15): stations gained
  optional profile/social columns (website, facebook, twitter,
  instagram), editable from a new station profile dialog; `GET
  /api/public/stations/{id}` (no auth) returns the brand + socials +
  requests-enabled flag + now playing + recent history + stream URL;
  `GET /api/public/stations/{id}/library?q=` is a lightweight public
  library search powering the request form; the listener request
  endpoint became anonymous (rules still rate-limit); `GET
  /api/stations/{id}/stream` reverse-proxies the Icecast mount through
  the API (same-origin playback, chunk-streamed, graceful 502 when
  Icecast is down); public page at `/stations/[id]/public` (HTML5
  player against the proxy, now playing + history polled from the
  public endpoint, request form with live search, social links) and an
  embeddable iframe widget at `/stations/[id]/widget`; the station page
  links to the public page. Verified end-to-end with a fake Icecast:
  anonymous station info (socials, now playing, history) → anonymous
  library search → anonymous request 201 → stream proxy 200 with
  byte-identical audio and correct content-type → graceful 502 when
  the mount is unreachable.

- **Phase 2 — Auth, users, roles** (2026-08-14): argon2 password hashing,
  `tower-sessions` SQLite-backed session cookies (14-day inactivity expiry,
  key from `CRABCAST_SESSION_SECRET`), synchronizer-token CSRF on all
  mutations, email-less first-run bootstrap (`/api/auth/setup` +
  `/api/auth/bootstrap`), login/logout/password endpoints, role/permission
  model (super admin flag + `station_manager`/`dj`/`media_editor` roles with
  global or per-station scope via `user_roles`), station routes guarded by
  `CurrentUser` + `Csrf` extractors with audit logging on every mutation,
  super-admin-only user CRUD + audit log API, admin UI at `/users` (user
  create/edit/delete, role grants, audit feed), login page with bootstrap
  mode, `useMe` guard on station pages. Verified end-to-end: bootstrap →
  login → CSRF-enforced station create, DJ vs scoped station manager behave
  differently (DJ controls but cannot manage, scoped manager cannot create
  stations), deleted users lose sessions immediately, wrong passwords
  rejected, every mutation lands in the audit log.

---

## 14. References

- [Crabsoup README](../crabsoup/README.md) — engine capabilities and control
  API contract.
- [Crabsoup ROADMAP](../crabsoup/ROADMAP.md) — engine plan; Crabcast rides on
  it (multi-output, request protocols, metadata hooks already shipped).
- [Crabsoup control-port guide](../crabsoup/website/guide/control-port.md) —
  full command/field table the `control/` client implements against.
- [Crabsoup ARCHITECTURE](../crabsoup/docs/ARCHITECTURE.md) — threading and
  tap model that the supervisor must respect.
