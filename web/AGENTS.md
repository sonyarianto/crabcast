# web/ — Vite + React SPA

The admin app, public station pages, and embeddable player widget. It is a
client-rendered SPA: no SSR, all data comes from `src/lib/api.ts` against
`/api/*` (relative paths — dev proxy in `vite.config.ts`, nginx in prod).

- **Stack**: Vite 7, React 19, TypeScript, Tailwind CSS v4, shadcn/ui
  (Base UI variant) under `src/components/ui/`, react-router v8.
- **Routes**: the whole route table lives in `src/main.tsx`; pages under
  `src/pages/` (station pages in `src/pages/station/`).
- **Commands**: `npm run dev` (port 3000, proxies `/api` to
  `API_UPSTREAM`), `npm run typecheck`, `npm run lint`, `npm run build`
  (→ `dist/`), `npm run check` (prettier).
- **PWA**: `public/manifest.webmanifest`, icons, and `public/sw.js`; the
  worker is registered in production only (`src/components/pwa-register.tsx`).
- **Bare-metal serving**: `serve.mjs` (Node built-ins only, SPA fallback +
  `/api` proxy) backs `packaging/crabcast-web.service`; Docker uses nginx
  (`../docker/nginx.conf.template`).
- Keep the API layer as the single source of truth — never fetch the DB
  directly from components.
