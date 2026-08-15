// Minimal production static server for the built SPA (dist/), used by the
// bare-metal systemd unit (packaging/crabcast-web.service). Serves assets,
// falls back to index.html for client-side routes, and reverse-proxies
// /api to API_UPSTREAM. Docker deployments use nginx instead (see
// docker/nginx.conf.template). Node built-ins only, so no node_modules
// are needed at runtime.
import { createServer } from "node:http";
import { request as httpRequest } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("./dist", import.meta.url));
const port = Number(process.env.PORT ?? 3000);
const upstream = process.env.API_UPSTREAM ?? "http://127.0.0.1:8080";
const upstreamUrl = new URL(upstream);

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".webmanifest": "application/manifest+json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
};

const server = createServer(async (req, res) => {
  const url = new URL(
    req.url ?? "/",
    `http://${req.headers.host ?? "localhost"}`,
  );

  if (url.pathname.startsWith("/api/")) {
    const target = new URL(url.pathname + url.search, upstreamUrl);
    const proxyReq = httpRequest(
      target,
      {
        method: req.method,
        headers: { ...req.headers, host: target.host },
      },
      (proxyRes) => {
        res.writeHead(proxyRes.statusCode ?? 502, proxyRes.headers);
        proxyRes.pipe(res);
      },
    );
    proxyReq.on("error", () => {
      res.writeHead(502, { "Content-Type": "text/plain" });
      res.end("Bad gateway");
    });
    req.pipe(proxyReq);
    return;
  }

  const pathname = decodeURIComponent(url.pathname);
  const filePath = normalize(join(root, pathname));
  if (!filePath.startsWith(root)) {
    res.writeHead(403);
    res.end("Forbidden");
    return;
  }
  try {
    let info = await stat(filePath);
    if (info.isDirectory()) {
      const index = join(filePath, "index.html");
      info = await stat(index);
      return serveFile(res, index, info);
    }
    return serveFile(res, filePath, info);
  } catch {
    // SPA fallback: let react-router handle the route.
    try {
      const index = join(root, "index.html");
      const data = await readFile(index);
      res.writeHead(200, {
        "Content-Type": MIME[".html"],
        "Cache-Control": "no-cache",
      });
      res.end(data);
    } catch {
      res.writeHead(500);
      res.end("Internal server error");
    }
  }
});

function serveFile(res, path, info) {
  const ext = extname(path);
  const immutable = ext === ".js" || ext === ".css";
  res.writeHead(200, {
    "Content-Type": MIME[ext] ?? "application/octet-stream",
    "Content-Length": info.size,
    "Cache-Control": immutable
      ? "public, max-age=31536000, immutable"
      : "no-cache",
  });
  readFile(path).then((data) => res.end(data));
}

server.listen(port, "0.0.0.0", () => {
  console.log(`Crabcast web serving ${root} on :${port}, API at ${upstream}`);
});
