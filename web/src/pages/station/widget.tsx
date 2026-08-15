"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "react-router";
import { Radio } from "lucide-react";

import { getPublicStation, type PublicStation } from "@/lib/api";

const POLL_MS = 10_000;

export default function StationWidget() {
  const params = useParams<{ id: string }>();
  const stationId = params.id!;

  const [station, setStation] = useState<PublicStation | null>(null);

  const load = useCallback(() => {
    getPublicStation(stationId)
      .then(setStation)
      .catch(() => setStation(null));
  }, [stationId]);

  useEffect(() => {
    load();
    const interval = setInterval(load, POLL_MS);
    return () => clearInterval(interval);
  }, [load]);

  return (
    <div className="flex min-h-dvh flex-col justify-between rounded-lg border bg-card p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Radio className="size-4 shrink-0" />
          <span className="truncate text-sm font-semibold">
            {station?.name ?? "…"}
          </span>
        </div>
        <span
          className={`size-2 shrink-0 rounded-full ${
            station?.now
              ? "animate-pulse bg-destructive"
              : "bg-muted-foreground/40"
          }`}
        />
      </div>
      <p className="my-2 truncate text-center text-sm font-medium">
        {station?.now?.title ?? "Off air"}
      </p>
      {station && (
        <audio src={station.stream_url} controls className="w-full" />
      )}
    </div>
  );
}
