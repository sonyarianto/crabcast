# web — Crabcast admin SPA

Vite + React + TypeScript + Tailwind CSS v4 + shadcn/ui. The web app is a
client-side SPA: every page calls the Rust API (`/api/*`) through
`src/lib/api.ts`; there is no server-side rendering.

## Commands

```sh
npm run dev        # Vite dev server on :3000, proxies /api to API_UPSTREAM
npm run typecheck  # tsc -b (noEmit)
npm run lint       # eslint (flat config)
npm run build      # tsc -b && vite build → dist/
npm run preview    # serve the built dist/ (vite preview)
npm run serve      # node serve.mjs (bare-metal static server + /api proxy)
npm run check      # prettier --check
```

## Layout

```
src/main.tsx        router + providers (theme, PWA registration)
src/pages/          route components (station/* for /stations/:id/...)
src/components/     shared + shadcn/ui components
src/lib/            api client (api.ts), auth hook (use-me.ts), utils
public/             static assets: icons, manifest.webmanifest, sw.js
vite.config.ts      dev proxy for /api, @ → src alias
serve.mjs           dependency-free static server (systemd bare-metal)
```

Production: static build in `dist/`, served by nginx in Docker
(`../docker/Dockerfile.web`) or `serve.mjs` on bare metal
(`../packaging/crabcast-web.service`).
