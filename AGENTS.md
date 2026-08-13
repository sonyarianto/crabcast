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
npm --prefix web run dev        # dev app on :3000
npm --prefix web run lint
npm --prefix web run build
npx tsc --noEmit                # (from web/)

# Full stack
make dev                        # docker compose: server + web + icecast
```

CI runs: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `npx tsc --noEmit`, `npm run lint`, `npm run build`.

## Conventions

- **Rust API is the single source of truth.** The web app never touches the
  DB; it calls `/api/*`, proxied through Next rewrites
  (`web/next.config.ts`, upstream from `API_UPSTREAM`, default
  `http://localhost:8080`).
- Server modules mirror the roadmap layout: `api/`, `auth/`, `stations/`,
  `lua/`, `control/`, `media/`, `analytics/`, `db/`. New phases add modules
  there.
- SQLite via sqlx; migrations live in `server/migrations/`, run at boot.
  Always add a migration for schema changes.
- Web uses shadcn/ui (Base UI variant) — reuse existing components under
  `web/components/ui/`; don't hand-roll widgets.
- No comments unless they explain *why*; no secrets in code or in generated
  `crabsoup.lua`.
- Phases land as working features with tests, and the ROADMAP checklist is
  updated in the same commit.

## Next.js 16 note

`web/` runs Next.js 16 with breaking changes vs older versions. Read the
bundled docs in `web/node_modules/next/dist/docs/` before writing Next code;
`web/AGENTS.md` has more detail.

## Engine

Crabsoup (sibling repo) is the streaming engine. Key integration points:
HTTP control API (`GET /status`, `POST /cmd`), Lua config generation, and
one supervised process per station. Read `../crabsoup/docs/ARCHITECTURE.md`
before touching station lifecycle code.
