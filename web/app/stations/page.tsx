"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { PlusIcon, Radio } from "lucide-react";

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
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { toast } from "@/components/ui/toast";
import {
  createStation,
  deleteStation,
  listStations,
  logout,
  type Station,
  type StationInput,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

type Status =
  | { state: "loading" }
  | { state: "ok"; stations: Station[] }
  | { state: "error"; message: string };

const defaultInput: StationInput = {
  name: "",
  description: "",
  playlist_dir: "",
  jingles_dir: "",
  sample_rate: 44100,
  channels: 2,
  frames_per_buffer: 4096,
  crossfade_seconds: 3,
  fade_curve: 1,
  duck_seconds: 1.5,
  harbor_port: 8005,
  harbor_mount: "/live",
  harbor_password: "dj",
  control_port: 1234,
  control_http_port: 9234,
  icecast_host: "localhost",
  icecast_port: 8000,
  icecast_mount: "/radio",
  icecast_format: "mp3",
  icecast_bitrate: 128000,
  icecast_source_user: "source",
  icecast_source_password: "hackme",
};

const FIELDS: {
  key: keyof StationInput;
  label: string;
  type: "text" | "number" | "password";
  placeholder?: string;
  colSpan?: string;
}[] = [
  { key: "name", label: "Name", type: "text", placeholder: "My radio" },
  {
    key: "description",
    label: "Description",
    type: "text",
    placeholder: "What this station plays",
    colSpan: "sm:col-span-2",
  },
  { key: "playlist_dir", label: "Playlist directory", type: "text" },
  { key: "jingles_dir", label: "Jingles directory", type: "text" },
  { key: "harbor_port", label: "Harbor (DJ) port", type: "number" },
  { key: "harbor_mount", label: "Harbor mount", type: "text" },
  { key: "harbor_password", label: "Harbor password", type: "password" },
  { key: "control_port", label: "Telnet port", type: "number" },
  { key: "control_http_port", label: "HTTP control port", type: "number" },
  { key: "icecast_host", label: "Icecast host", type: "text" },
  { key: "icecast_port", label: "Icecast port", type: "number" },
  { key: "icecast_mount", label: "Icecast mount", type: "text" },
  { key: "icecast_format", label: "Format (mp3/opus)", type: "text" },
  { key: "icecast_bitrate", label: "Bitrate (bps)", type: "number" },
  { key: "icecast_source_user", label: "Icecast source user", type: "text" },
  {
    key: "icecast_source_password",
    label: "Icecast source password",
    type: "password",
  },
];

export default function StationsPage() {
  const { meState, refresh } = useMe();
  const [status, setStatus] = useState<Status>({ state: "loading" });
  const [form, setForm] = useState<StationInput>(defaultInput);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(() => {
    setStatus({ state: "loading" });
    listStations()
      .then((stations) => setStatus({ state: "ok", stations }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, []);

  useEffect(() => {
    listStations()
      .then((stations) => setStatus({ state: "ok", stations }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, []);

  const submit = async () => {
    setSaving(true);
    try {
      await createStation(form);
      toast.add({
        title: "Station created",
        type: "success",
        timeout: 3000,
      });
      setForm(defaultInput);
      reload();
    } catch (err) {
      toast.add({
        title: "Failed to create station",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const remove = async (station: Station) => {
    if (!confirm(`Delete "${station.name}"? Its engine will be stopped.`)) {
      return;
    }
    try {
      await deleteStation(station.id);
      toast.add({ title: "Station deleted", type: "success", timeout: 3000 });
      reload();
    } catch (err) {
      toast.add({
        title: "Failed to delete station",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          {meState.state === "ready" && meState.me.user.is_super_admin && (
            <Button variant="ghost" size="sm" render={<Link href="/users" />}>
              Users
            </Button>
          )}
          <Button variant="ghost" size="sm" render={<Link href="/library" />}>
            Library
          </Button>
          {meState.state === "ready" && (
            <>
              <span className="text-sm text-muted-foreground">
                {meState.me.user.display_name || meState.me.user.username}
              </span>
              <Button
                variant="ghost"
                size="sm"
                onClick={async () => {
                  await logout();
                  refresh();
                }}
              >
                Log out
              </Button>
            </>
          )}
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-8">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">Stations</h1>
            <p className="text-sm text-muted-foreground">
              One supervised Crabsoup engine per station.
            </p>
          </div>
          <Dialog>
            <DialogTrigger render={<Button />}>
              <PlusIcon />
              New station
            </DialogTrigger>
            <DialogContent className="sm:max-w-lg">
              <DialogHeader>
                <DialogTitle>New station</DialogTitle>
                <DialogDescription>
                  The engine is validated with `crabsoup --check` before the
                  station starts.
                </DialogDescription>
              </DialogHeader>
              <div className="grid gap-4 sm:grid-cols-2">
                {FIELDS.map((field) => (
                  <div
                    key={field.key}
                    className={field.colSpan ?? "grid gap-2"}
                  >
                    <Label htmlFor={field.key}>{field.label}</Label>
                    <input
                      id={field.key}
                      type={field.type}
                      value={String(form[field.key] ?? "")}
                      onChange={(e) =>
                        setForm((f) => ({
                          ...f,
                          [field.key]:
                            field.type === "number"
                              ? Number(e.target.value)
                              : e.target.value,
                        }))
                      }
                      placeholder={field.placeholder}
                      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    />
                  </div>
                ))}
              </div>
              <DialogFooter>
                <Button onClick={submit} disabled={saving}>
                  {saving ? "Starting engine…" : "Create station"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        {status.state === "loading" && (
          <p className="text-sm text-muted-foreground">Loading…</p>
        )}
        {status.state === "error" && (
          <p className="text-sm text-destructive">{status.message}</p>
        )}
        {status.state === "ok" && status.stations.length === 0 && (
          <Card>
            <CardHeader>
              <CardTitle>No stations yet</CardTitle>
              <CardDescription>
                Create your first station to spin up an engine.
              </CardDescription>
            </CardHeader>
          </Card>
        )}
        {status.state === "ok" &&
          status.stations.map((station) => (
            <Card key={station.id} className="mb-4">
              <CardHeader>
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <CardTitle className="text-base">{station.name}</CardTitle>
                    <CardDescription>
                      {station.description || station.id}
                    </CardDescription>
                  </div>
                  <div className="flex shrink-0 gap-2">
                    <Button variant="outline" size="sm" render={<Link href={`/stations/${station.id}`} />}>
                      Details
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => remove(station)}
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="text-sm text-muted-foreground">
                {station.icecast_mount} @ {station.icecast_host}:
                {station.icecast_port}
              </CardContent>
            </Card>
          ))}
      </main>
    </div>
  );
}