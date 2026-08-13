"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { ArrowLeftIcon, PlayIcon, SkipForwardIcon, Radio } from "lucide-react";

import { ThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@/components/ui/toast";
import {
  getStation,
  getStationStatus,
  sendCommand,
  type SongHistory,
  type Station,
  type StationStatus,
} from "@/lib/api";

const STATUS_POLL_MS = 15_000;

type Loaded = {
  station: Station;
  status: StationStatus | null;
};

export default function StationPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;

  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [nowPlaying, setNowPlaying] = useState<string | null>(null);
  const [history, setHistory] = useState<SongHistory[]>([]);
  const retryRef = useRef(0);

  // The retry loop needs the latest callback without referencing itself
  // during its own declaration, so it goes through a ref.
  const refreshRef = useRef<(scheduleRetry: boolean) => void>(() => {});

  const refreshStatus = useCallback((scheduleRetry: boolean) => {
    getStationStatus(id)
      .then((status) => {
        retryRef.current = 0;
        setLoaded((prev) => {
          if (!prev) return prev;
          return { station: prev.station, status };
        });
        setNowPlaying(status.playing ?? null);
        if (status.history.length) setHistory(status.history);
      })
      .catch((err: unknown) => {
        const message = err instanceof Error ? err.message : "Unknown error";
        setError(message);
        if (!scheduleRetry) return;
        // A short backoff keeps the polling alive across engine restarts
        // without spamming the log when the API itself is down.
        retryRef.current = Math.min(retryRef.current + 1, 5);
        setTimeout(() => refreshRef.current(true), retryRef.current * 2000);
      });
  }, [id]);

  useEffect(() => {
    refreshRef.current = refreshStatus;
  }, [refreshStatus]);

  useEffect(() => {
    let cancelled = false;

    getStation(id)
      .then((station) => {
        if (cancelled) return;
        setLoaded({ station, status: null });
        setError(null);
      })
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );

    const interval = setInterval(() => refreshStatus(true), STATUS_POLL_MS);

    // SSE: authoritative track-change stream from the engine webhook.
    const events = new EventSource(`/api/stations/${id}/events`);
    events.addEventListener("message", (ev) => {
      try {
        const payload = JSON.parse((ev as MessageEvent).data) as {
          type: string;
          data?: { title?: string; started_at?: string; state?: string };
        };
        if (payload.type === "Track" && payload.data?.title) {
          setNowPlaying(payload.data.title);
          const entry: SongHistory = {
            id: Date.now(),
            station_id: id,
            title: payload.data.title,
            started_at: payload.data.started_at ?? new Date().toISOString(),
            ended_at: null,
          };
          setHistory((prev) => [entry, ...prev].slice(0, 50));
        }
      } catch {
        // malformed frame; ignore and wait for the next one
      }
    });

    return () => {
      cancelled = true;
      clearInterval(interval);
      events.close();
    };
  }, [id, refreshStatus]);

  const run = async (command: string) => {
    try {
      await sendCommand(id, command);
      toast.add({
        title: `Command sent: ${command}`,
        type: "success",
        timeout: 3000,
      });
      setTimeout(() => refreshStatus(true), 1500);
    } catch (err) {
      toast.add({
        title: "Command failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  if (error && !loaded) {
    return (
      <Shell>
        <p className="text-sm text-destructive">{error}</p>
      </Shell>
    );
  }
  if (!loaded) {
    return (
      <Shell>
        <p className="text-sm text-muted-foreground">Loading…</p>
      </Shell>
    );
  }

  const { station, status } = loaded;
  const process = status?.process ?? "stopped";

  return (
    <Shell>
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div>
          <Link
            href="/stations"
            className="mb-2 inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
          >
            <ArrowLeftIcon className="size-4" />
            All stations
          </Link>
          <h1 className="text-2xl font-semibold tracking-tight">
            {station.name}
          </h1>
          <p className="text-sm text-muted-foreground">
            {station.description || station.id}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => run("skip")}
            disabled={process !== "running"}
          >
            <SkipForwardIcon />
            Skip
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => run("jingles.play")}
            disabled={process !== "running"}
          >
            <PlayIcon />
            Play jingle
          </Button>
        </div>
      </div>

      <Card className="mb-4">
        <CardHeader>
          <CardTitle className="text-base">Live status</CardTitle>
          <CardDescription>
            Process supervision, engine control port, and the current track.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap items-center gap-x-8 gap-y-4">
            <div className="flex items-center gap-3">
              <span
                className={`size-3 rounded-full ${
                  process === "running"
                    ? "bg-emerald-500"
                    : process === "failed"
                      ? "bg-destructive"
                      : "bg-muted-foreground/50"
                }`}
              />
              <span className="text-sm font-medium">{process}</span>
            </div>
            <div className="text-sm">
              <span className="text-muted-foreground">pid </span>
              <span className="font-medium">{status?.pid ?? "—"}</span>
            </div>
            <div className="text-sm">
              <span className="text-muted-foreground">engine uptime </span>
              <span className="font-medium">
                {status?.engine_uptime_seconds != null
                  ? `${status.engine_uptime_seconds}s`
                  : "—"}
              </span>
            </div>
            <div className="text-sm">
              <span className="text-muted-foreground">restarts </span>
              <span className="font-medium">{status?.restarts ?? 0}</span>
            </div>
            {status?.last_error && (
              <div className="text-sm text-destructive">
                {status.last_error}
              </div>
            )}
          </div>
          <div className="mt-4 rounded-lg bg-muted/50 p-4">
            <p className="text-xs text-muted-foreground">NOW PLAYING</p>
            <p className="mt-1 truncate font-medium">
              {nowPlaying ?? "Idle — nothing playing"}
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Recent history</CardTitle>
          <CardDescription>
            Tracks reported by the engine webhook.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {history.length === 0 ? (
            <p className="text-sm text-muted-foreground">No tracks yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Title</TableHead>
                  <TableHead>Started</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {history.map((entry) => (
                  <TableRow key={entry.id}>
                    <TableCell className="max-w-md truncate">
                      {entry.title}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-muted-foreground">
                      {new Date(entry.started_at).toLocaleTimeString()}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </Shell>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <ThemeToggle />
      </header>
      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  );
}