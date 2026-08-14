"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "next/navigation";
import { Link2, Radio } from "lucide-react";

import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import {
  createRequest,
  getPublicStation,
  searchPublicLibrary,
  type PublicLibraryHit,
  type PublicStation,
} from "@/lib/api";

const POLL_MS = 10_000;

export default function PublicStationPage() {
  const params = useParams<{ id: string }>();
  const stationId = params.id;

  const [station, setStation] = useState<PublicStation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PublicLibraryHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [requested, setRequested] = useState<Set<string>>(new Set());

  const load = useCallback(() => {
    getPublicStation(stationId)
      .then(setStation)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );
  }, [stationId]);

  useEffect(() => {
    load();
    const interval = setInterval(load, POLL_MS);
    return () => clearInterval(interval);
  }, [load]);

  // Debounced library search for the request form, driven from the input
  // handler (setState in an effect is not allowed by the lint rules).
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onQueryChange = (value: string) => {
    setQuery(value);
    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (!value.trim()) {
      setResults([]);
      return;
    }
    searchTimer.current = setTimeout(() => {
      setSearching(true);
      searchPublicLibrary(stationId, value.trim())
        .then(setResults)
        .catch(() => setResults([]))
        .finally(() => setSearching(false));
    }, 250);
  };

  const request = async (hit: PublicLibraryHit) => {
    try {
      await createRequest(stationId, hit.id);
      setRequested((prev) => new Set(prev).add(hit.id));
      toast.add({
        title: `Requested: ${hit.title || hit.filename}`,
        type: "success",
        timeout: 3000,
      });
    } catch (err) {
      toast.add({
        title: "Request failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const socials: { label: string; url: string }[] = station
    ? [
        { label: "Website", url: station.website },
        { label: "Facebook", url: station.facebook },
        { label: "X", url: station.twitter },
        { label: "Instagram", url: station.instagram },
      ].filter((s) => s.url.trim())
    : [];

  if (error && !station) {
    return (
      <div className="flex min-h-dvh flex-col items-center justify-center gap-3 p-6">
        <p className="text-sm text-destructive">{error}</p>
        <p className="text-sm text-muted-foreground">
          This station may not exist or is not broadcasting yet.
        </p>
      </div>
    );
  }
  if (!station) {
    return (
      <div className="flex min-h-dvh items-center justify-center p-6">
        <p className="text-sm text-muted-foreground">Loading…</p>
      </div>
    );
  }

  return (
    <div className="min-h-dvh">
      <header className="border-b px-4 py-3">
        <div className="mx-auto flex max-w-2xl items-center justify-between">
          <div className="flex items-center gap-2 font-semibold">
            <Radio className="size-5" />
            {station.name}
          </div>
          <div className="flex items-center gap-3 text-sm text-muted-foreground">
            {socials.map((s) => (
              <a
                key={s.label}
                href={s.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 hover:text-foreground"
              >
                <Link2 className="size-3" />
                {s.label}
              </a>
            ))}
          </div>
        </div>
      </header>

      <main className="mx-auto grid w-full max-w-2xl gap-6 px-4 py-8">
        {station.description && (
          <p className="text-sm text-muted-foreground">
            {station.description}
          </p>
        )}

        {/* Player */}
        <div className="rounded-xl border bg-card p-5">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-xs font-medium tracking-wide text-muted-foreground">
              NOW PLAYING
            </span>
            <span
              className={`size-2 rounded-full ${
                station.now ? "animate-pulse bg-destructive" : "bg-muted-foreground/40"
              }`}
            />
          </div>
          <p className="mb-4 truncate text-lg font-semibold">
            {station.now?.title ?? "Off air — check back soon"}
          </p>
          <audio src={station.stream_url} controls className="w-full" />
        </div>

        {/* Request form */}
        {station.requests_enabled && (
          <section className="rounded-xl border bg-card p-5">
            <h2 className="mb-1 text-sm font-semibold">Request a song</h2>
            <p className="mb-3 text-xs text-muted-foreground">
              Search the library and request a track — it plays on air within
              seconds.
            </p>
            <input
              value={query}
              onChange={(e) => onQueryChange(e.target.value)}
              placeholder="Search artist or title…"
              className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
            <ul className="mt-3 divide-y">
              {searching && (
                <li className="py-2 text-sm text-muted-foreground">
                  Searching…
                </li>
              )}
              {!searching && query.trim() && results.length === 0 && (
                <li className="py-2 text-sm text-muted-foreground">
                  No matches.
                </li>
              )}
              {results.map((hit) => (
                <li
                  key={hit.id}
                  className="flex items-center justify-between gap-3 py-2"
                >
                  <div className="min-w-0">
                    <p className="truncate font-medium">
                      {hit.title || hit.filename}
                    </p>
                    {hit.artist && (
                      <p className="truncate text-xs text-muted-foreground">
                        {hit.artist}
                      </p>
                    )}
                  </div>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={requested.has(hit.id)}
                    onClick={() => request(hit)}
                  >
                    {requested.has(hit.id) ? "Requested" : "Request"}
                  </Button>
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* History */}
        <section>
          <h2 className="mb-2 text-sm font-semibold">Recently played</h2>
          {station.history.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Nothing has played yet.
            </p>
          ) : (
            <ul className="divide-y">
              {station.history.map((h) => (
                <li
                  key={h.id}
                  className="flex items-center justify-between gap-3 py-1.5 text-sm"
                >
                  <span className="truncate">{h.title}</span>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {new Date(h.started_at).toLocaleTimeString()}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      </main>
    </div>
  );
}
