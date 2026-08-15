"use client";

import { useEffect } from "react";

// Registers the PWA service worker in production only (dev HMR conflicts
// with a worker). Returns null — it has no UI.
export function PwaRegister() {
  useEffect(() => {
    if (process.env.NODE_ENV !== "production") return;
    if (!("serviceWorker" in navigator)) return;
    navigator.serviceWorker.register("/sw.js").catch(() => {
      // Registration is best-effort; never take the app down over it.
    });
  }, []);
  return null;
}
