import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// SPA dev server: serves the admin app and proxies /api to the Rust API
// (mirrors what Next rewrites did). In production, nginx does the same.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  server: {
    port: 3000,
    proxy: {
      "/api": {
        target: process.env.API_UPSTREAM ?? "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
});
