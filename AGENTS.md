# AGENTS.md — Crabcast

Guidance for AI agents working in this repo. Follow the roadmap and
conventions below; when in doubt, mirror what the sibling
[`../crabsoup`](../crabsoup) repo does.

## Commands

```sh
# Rust (server/)
cargo fmt --manifest-path server/Cargo.toml            # format
cargo clippy --manifest-path server/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path server/Cargo.toml
cargo run --manifest-path server/Cargo.toml            # dev API on :8080

# Web (web/)
npm --prefix web run dev        # Vite dev app on :3000 (proxies /api to :8080)
npm --prefix web run typecheck  # tsc -b
npm --prefix web run lint
npm --prefix web run build      # tsc -b && vite build -> dist/

# Full stack
make dev                        # docker compose: server + web + icecast
```

CI runs: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `npm run typecheck`, `npm run lint`, `npm run build`.

## Conventions

- **Rust API is the single source of truth.** The web app never touches the
  DB; it calls `/api/*` with relative paths. In dev, `web/vite.config.ts`
  proxies `/api` to `API_UPSTREAM` (default `http://localhost:8080`); in
  production nginx does the same.
- Server modules mirror the roadmap layout: `api/`, `auth/`, `stations/`,
  `lua/`, `control/`, `media/`, `analytics/`, `db/`. New phases add modules
  there.
- SQLite via sqlx; migrations live in `server/migrations/`, run at boot.
  Always add a migration for schema changes.
- Web uses shadcn/ui (Base UI variant) — reuse existing components under
  `web/src/components/ui/`; don't hand-roll widgets.
- No comments unless they explain *why*; no secrets in code or in generated
  `crabsoup.lua`.
- Phases land as working features with tests, and the ROADMAP checklist is
  updated in the same commit.

## Web app note

`web/` is a Vite + React SPA (TypeScript, Tailwind v4, shadcn/ui, react-router).
No SSR: all data flows through the client-side API layer (`web/src/lib/api.ts`)
against `/api/*` — keep it that way.

## Engine

Crabsoup (sibling repo) is the streaming engine. Key integration points:
HTTP control API (`GET /status`, `POST /cmd`), Lua config generation, and
one supervised process per station. Read `../crabsoup/docs/ARCHITECTURE.md`
before touching station lifecycle code.
