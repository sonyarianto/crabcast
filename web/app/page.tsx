"use client";

import { useEffect, useState } from "react";
import { Radio } from "lucide-react";

import { ThemeToggle } from "@/components/theme-toggle";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { fetchHealth, type Health } from "@/lib/api";

type Status =
  | { state: "loading" }
  | { state: "ok"; health: Health }
  | { state: "error"; message: string };

export default function Home() {
  const [status, setStatus] = useState<Status>({ state: "loading" });

  useEffect(() => {
    const controller = new AbortController();
    fetchHealth(controller.signal)
      .then((health) => setStatus({ state: "ok", health }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
    return () => controller.abort();
  }, []);

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <ThemeToggle />
      </header>

      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col justify-center gap-8 px-4 py-16">
        <div className="space-y-2">
          <h1 className="text-3xl font-semibold tracking-tight">
            Radio management, in Rust
          </h1>
          <p className="text-muted-foreground">
            Crabcast is an AzuraCast-style platform: multi-station, playlist
            automation, live DJ support, requests, and analytics — powered by
            the Crabsoup engine.
          </p>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>API health</CardTitle>
            <CardDescription>
              Live check against the Rust backend (axum + SQLite).
            </CardDescription>
          </CardHeader>
          <CardContent className="flex items-center gap-3">
            <span
              className={`size-2.5 rounded-full ${
                status.state === "ok"
                  ? "bg-emerald-500"
                  : status.state === "loading"
                    ? "animate-pulse bg-amber-500"
                    : "bg-destructive"
              }`}
            />
            {status.state === "loading" && (
              <span className="text-sm text-muted-foreground">
                Checking API…
              </span>
            )}
            {status.state === "ok" && (
              <span className="text-sm">
                <span className="font-medium">{status.health.status}</span>
                <span className="text-muted-foreground">
                  {" · "}v{status.health.version}
                  {" · "}db {status.health.db}
                </span>
              </span>
            )}
            {status.state === "error" && (
              <span className="text-sm text-destructive">
                API unreachable — is the server running? ({status.message})
              </span>
            )}
          </CardContent>
        </Card>

        <p className="text-sm text-muted-foreground">
          Phase 0 scaffold. Station control, media, and the live dashboard land
          in the next phases.
        </p>
      </main>
    </div>
  );
}
