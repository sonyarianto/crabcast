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

- [ ] `lua/` generator: station model → `crabsoup.lua` (mounts, playlist
      directory, jingles dir, harbor, sample rate, crossfade/duck settings).
      `--check` on every generation; diff + restart on change.
- [ ] `stations/` supervisor: spawn one `crabsoup` per station, capture logs,
      restart with backoff on crash, status in API (`/api/stations/:id/status`).
- [ ] `control/` client: `/status`, `/uptime`, `/queue`, `/jingles`, `/cmd`
      (`skip`, `queue.push`, `jingles.play`) with the `{"ok": ...}` envelope.
- [ ] Track-change events: **small Crabsoup addition** — `on_metadata` webhook
      POST (or SSE) to the backend so song history is pushed, not polled.
      Record `now_playing` + history rows in DB.
- [ ] Minimal station CRUD + a station dashboard page showing live status and
      now-playing over SSE.

**Acceptance**: create a station in the UI, upload a playlist folder path, hear
it on an Icecast mount, skip a track, see now-playing update in real time.

### Phase 2 — Auth, users, roles

- [ ] Session auth (argon2, secure cookies, CSRF), login/logout, change
      password, email-less first-run admin bootstrap.
- [ ] Role/permission model (AzuraCast parity): global roles (super admin,
      station manager, DJ, media editor) + per-station permissions.
- [ ] User CRUD in admin UI; invite by email later (Phase 10).
- [ ] Audit log (who changed what) — cheap with SQLite, worth it from day one.

**Acceptance**: two users with different station permissions behave
accordingly; every mutation is audited.

### Phase 3 — Media library

- [ ] Upload (drag & drop, resumable chunks), storage trait (local FS now,
      S3 later), dedupe by content hash.
- [ ] Tag scan: title/artist/album/genre, duration, cover art, replaygain tags
      (the engine reads these — surface them). Tag editing writes back.
- [ ] Waveform + audio preview in the browser (no download-to-check).
- [ ] Search + filters + virtualized list; bulk edit; "add to playlist" from
      results.
- [ ] Library page with column sorting and cover-art grid toggle.

**Acceptance**: upload 1,000 tracks, browse/filter/edit them at p95 < 50 ms,
attach a folder to a station playlist and hear it on air.

### Phase 4 — Playlists & scheduling (AutoDJ)

- [ ] Playlist types: standard (shuffle/sequential), looping, scheduled
      (dayparted), once-per-hour (AzuraCast parity), request playlist.
- [ ] Drag-and-drop ordering, per-playlist weights, per-track fade/cue
      overrides (maps to Crabsoup `cue_cut`/`annotate:`).
- [ ] Scheduler UI: time-of-day + weekday rules → Crabsoup `switch`/`rotate`
      generation with live preview of the generated Lua.
- [ ] Crossfade/ducking/DSP station settings mapped to Crabsoup `set()` knobs
      and `normalize(replaygain(...))`.

**Acceptance**: a station with dayparting + crossfades runs unattended for 24 h
with a correct schedule; changing a rule applies live without dropping audio.

### Phase 5 — Streamers (live DJ)

- [ ] Streamer accounts + mount config (`input.harbor` source password).
- [ ] Connection tracking (on-air/off-air via harbor state), on-air indicator
      in the dashboard, ducking visualization.
- [ ] Streamer-facing view: connect instructions (Icecast source client),
      mic test, disconnect.

**Acceptance**: a DJ connects from a source client, the playlist ducks out, and
it fades back in on disconnect — all visible in real time.

### Phase 6 — Requests & jingles

- [ ] Request system: configurable request playlists + per-station request
      rules (max per hour, dedupe, moderation toggle); backend maps to
      `queue.push`.
- [ ] Jingles management UI: upload, preview, trigger from admin; maps to
      `jingles.play`.
- [ ] Remote control surface: skip, queue, jingles from the dashboard and
      (later) from a mobile PWA page.

**Acceptance**: a listener request plays within seconds; a jingle fires on
command; abuse rules hold.

### Phase 7 — Public pages & web player

- [ ] Per-station public page (brandable): player, now-playing art, song
      history, request form, listener count, social links.
- [ ] Embeddable widget (iframe) for third-party sites.
- [ ] Web player: native HTML5 audio against the mount (MP3 for max
      compatibility; Opus mount when available); SSE-driven metadata overlay.
- [ ] Public API endpoint for third parties (now-playing, history) — the
      AzuraCast API-parity seed (full REST API in Phase 9).

**Acceptance**: a visitor can play the stream, see what's on now, and request a
song — all without an account.

### Phase 8 — Analytics & monitoring

- [ ] Listener tracking: poll Icecast admin API per mount, store per-minute
      samples; unique-listeners approximation.
- [ ] Station dashboard charts: listeners over time, top songs, request rates,
      uptime; song history export (CSV).
- [ ] Alerts: dead-air (`blank.detect` on_blank webhook), engine crash loops,
      disk usage, Icecast unreachable — email/webhook notifications.
- [ ] Uptime/history retention policy (configurable).

**Acceptance**: 7-day listener graph matches Icecast's own numbers within
tolerance; a forced dead-air episode raises an alert.

### Phase 9 — Performance, scale & API

- [ ] Benchmarks (criterion in `server/` + load tests): API p95, station
      startup, CPU/RAM per station at idle and under playout. Re-baseline
      against AzuraCast's known numbers (documented in this file).
- [ ] Postgres feature flag (sqlx) for multi-host deployments; shared
      SSE/event bus via Redis pub/sub (optional, behind the flag).
- [ ] CDN-friendly static serving for media/cover art; cache headers.
- [ ] Full REST API (AzuraCast-compatible surface where sensible) + API tokens.
- [ ] Horizontal-scale story: N API hosts, M station hosts, one shared DB.

**Acceptance**: 50 stations on a small VPS with p95 API < 50 ms and idle CPU
per station in single digits; documented benchmark table.

### Phase 10 — Packaging, deployment & docs

- [ ] Production Docker images (multi-stage, slim), `compose.prod.yml`,
      one-command install script (Debian/Ubuntu + Docker).
- [ ] Systemd unit (bare-metal install without Docker), upgrade path with
      DB migrations run automatically.
- [ ] Backup/restore (DB + media + station configs) from the admin UI.
- [ ] Onboarding wizard: first-run admin, create first station, add media,
      go live in < 5 minutes.
- [ ] Docs site (mirror Crabsoup's VitePress site pattern): getting started,
      station guide, engine reference, API reference.

**Acceptance**: fresh VPS → on-air station in under 10 minutes from the
install script; backup → restore verified in CI.

### Phase 11 — Stretch goals (post-1.0)

- [ ] Podcasts (AzuraCast parity): upload episodes, feed generation.
- [ ] HLS streaming (low-latency) as an alternative to raw mounts.
- [ ] PWA admin + mobile remote control.
- [ ] i18n: full translation pass (next-intl), RTL support.
- [ ] Built-in mount server (skip Icecast) — only after Phase 8/9 listener
      metrics justify the engine work; this is the biggest lever for the
      "better performance" claim but also the biggest risk, so it stays last.
- [ ] Plugins/webhooks out (Slack/Discord on-air notifications, etc.).

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

Benchmark methodology: criterion micro-benchmarks in `server/`, `oha`/`k6`
load tests in CI, and a `scripts/bench-station.sh` that spins up N stations and
records CPU/RAM over 10 minutes.

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

- (empty — populate from Phase 1 on)

---

## Done

- **Phase 0 — Scaffold & dev environment** (2026-08-14): monorepo layout
  (`server/` axum + SQLite, `web/` Next.js 16 + Tailwind v4 + shadcn/ui Base
  UI), `docker/compose.yml` + `make dev`, GitHub Actions CI (fmt/clippy/test,
  tsc/eslint/prettier/build), README + AGENTS.md. API health is visible from
  the web home page through Next rewrites (`/api/*` → `API_UPSTREAM`).

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
