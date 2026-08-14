"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { BellRing, Play, Trash, Upload } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { toast } from "@/components/ui/toast";
import {
  deleteJingle,
  listJingles,
  sendCommand,
  uploadJingles,
  type Jingle,
} from "@/lib/api";

export function JinglesCard({ stationId }: { stationId: string }) {
  const [jingles, setJingles] = useState<Jingle[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const reload = useCallback(() => {
    listJingles(stationId)
      .then(setJingles)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );
  }, [stationId]);

  useEffect(() => {
    reload();
  }, [reload]);

  const pickFiles = () => fileRef.current?.click();

  const upload = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setUploading(true);
    try {
      const res = await uploadJingles(stationId, Array.from(files));
      toast.add({
        title: `Uploaded ${res.uploaded.length} jingle(s)`,
        type: "success",
        timeout: 3000,
      });
      reload();
    } catch (err) {
      toast.add({
        title: "Upload failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setUploading(false);
      if (fileRef.current) fileRef.current.value = "";
    }
  };

  const play = async (filename: string) => {
    try {
      await sendCommand(stationId, `jingles.play ${filename}`);
      toast.add({
        title: `Jingle queued: ${filename}`,
        type: "success",
        timeout: 3000,
      });
    } catch (err) {
      toast.add({
        title: "Play failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const remove = async (filename: string) => {
    try {
      await deleteJingle(stationId, filename);
      toast.add({
        title: `Deleted ${filename}`,
        type: "success",
        timeout: 3000,
      });
      reload();
    } catch (err) {
      toast.add({
        title: "Delete failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  return (
    <Card className="mt-4">
      <CardHeader>
        <CardTitle className="flex items-center justify-between text-base">
          <span className="flex items-center gap-2">
            <BellRing className="size-4" />
            Jingles
          </span>
          <div className="flex gap-1">
            <input
              ref={fileRef}
              type="file"
              accept="audio/*"
              multiple
              className="hidden"
              onChange={(e) => upload(e.target.files)}
            />
            <Button
              variant="outline"
              size="sm"
              onClick={pickFiles}
              disabled={uploading}
            >
              <Upload />
              {uploading ? "Uploading…" : "Upload"}
            </Button>
          </div>
        </CardTitle>
        <CardDescription>
          Audio files in the station jingles folder. Uploads re-scan the
          engine so they are playable immediately; the header Play jingle
          button fires a random one on air.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {error && <p className="text-sm text-destructive">{error}</p>}
        {jingles === null && !error ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : jingles && jingles.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No jingles yet — upload some to trigger on air.
          </p>
        ) : (
          <ul className="divide-y">
            {jingles?.map((j) => (
              <li
                key={j.filename}
                className="flex items-center justify-between gap-4 py-2.5"
              >
                <div className="flex min-w-0 items-center gap-3">
                  <audio
                    src={`/api/stations/${stationId}/jingles/${encodeURIComponent(j.filename)}`}
                    preload="none"
                    controls
                    className="h-8 w-44"
                  />
                  <div className="min-w-0">
                    <p className="truncate font-medium">{j.filename}</p>
                    <p className="text-xs text-muted-foreground">
                      {(j.size_bytes / 1024).toFixed(1)} KB
                    </p>
                  </div>
                </div>
                <div className="flex shrink-0 gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => play(j.filename)}
                  >
                    <Play />
                    On air
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-destructive hover:text-destructive"
                    onClick={() => remove(j.filename)}
                  >
                    <Trash />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
