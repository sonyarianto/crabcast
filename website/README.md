# Crabcast website

VitePress documentation site (mirrors the Crabsoup site pattern).

## Local development

```sh
cd website
npm install
npm run dev      # http://localhost:5173
npm run build    # static site → .vitepress/dist
npm run preview  # preview the built site
```

## Deploying on Vercel

The site is a static build, so Vercel just needs the right root and
settings. Two options:

**Option A — import with Root Directory set to `website`:**

1. Import the repo (or connect the Git repo) in the Vercel dashboard.
2. Set **Root Directory** to `website`.
3. `vercel.json` already pins the framework (`vitepress`), build command
   (`npm run build`) and output directory (`.vitepress/dist`) — no other
   config needed.
4. Deploy. The site is fully static (no env vars, no server functions).

**Option B — CLI from the repo root:**

```sh
npm install --prefix website
npx vercel deploy --cwd website --prod
```

Note: the domain currently served is whatever you configure in the Vercel
dashboard — this repo is a monorepo, so the `website/` subdirectory is the
only thing deployed here.
