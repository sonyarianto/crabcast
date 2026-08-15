"use client";

import { useEffect, useState } from "react";
import {
  MicVocal,
  Pencil,
  Plus,
  RadioTower,
  Terminal,
  Trash,
  Users,
} from "lucide-react";

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
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { toast } from "@/components/ui/toast";
import {
  createStreamer,
  deleteStreamer,
  getStreamerConnectInfo,
  listStreamers,
  updateStreamer,
  type Streamer,
  type StreamerConnectInfo,
  type StreamerInput,
} from "@/lib/api";

type FormState = {
  name: string;
  description: string;
  source_password: string;
  enabled: boolean;
};

const EMPTY_FORM: FormState = {
  name: "",
  description: "",
  source_password: "",
  enabled: true,
};

export function StreamersCard({
  stationId,
  live,
}: {
  stationId: string;
  /** True while a DJ holds the harbor; the playlist is ducked. */
  live: boolean;
}) {
  const [streamers, setStreamers] = useState<Streamer[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Streamer | null>(null);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [connectInfo, setConnectInfo] = useState<StreamerConnectInfo | null>(
    null,
  );

  const reload = () => {
    listStreamers(stationId)
      .then(setStreamers)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );
  };

  useEffect(() => {
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stationId]);

  const openCreate = () => {
    setEditing(null);
    setForm(EMPTY_FORM);
    setFormOpen(true);
  };

  const openEdit = (s: Streamer) => {
    setEditing(s);
    setForm({
      name: s.name,
      description: s.description,
      source_password: "",
      enabled: s.enabled,
    });
    setFormOpen(true);
  };

  const save = async () => {
    if (!form.name.trim()) {
      toast.add({ title: "Name is required", type: "error", timeout: 4000 });
      return;
    }
    const input: StreamerInput = {
      name: form.name.trim(),
      description: form.description,
      source_password: form.source_password,
      enabled: form.enabled,
    };
    try {
      if (editing) {
        await updateStreamer(editing.id, input);
        toast.add({
          title: "Streamer updated",
          type: "success",
          timeout: 3000,
        });
      } else {
        await createStreamer(stationId, input);
        toast.add({
          title: "Streamer created",
          type: "success",
          timeout: 3000,
        });
      }
      setFormOpen(false);
      reload();
    } catch (err) {
      toast.add({
        title: "Save failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const remove = async (s: Streamer) => {
    try {
      await deleteStreamer(s.id);
      toast.add({
        title: `Streamer ${s.name} deleted`,
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

  const openConnect = async (s: Streamer) => {
    try {
      setConnectInfo(await getStreamerConnectInfo(s.id));
    } catch (err) {
      toast.add({
        title: "Connect info failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center justify-between text-base">
          <span className="flex items-center gap-2">
            <Users className="size-4" />
            Streamers
            {live && (
              <span className="text-destructive-foreground inline-flex animate-pulse items-center gap-1 rounded-full bg-destructive px-2 py-0.5 text-xs font-semibold">
                <span className="size-1.5 rounded-full bg-current" />
                LIVE
              </span>
            )}
          </span>
          <Button variant="outline" size="sm" onClick={openCreate}>
            <Plus />
            Add streamer
          </Button>
        </CardTitle>
        <CardDescription>
          Live DJ accounts — each has its own source password for the station
          mount. When a DJ connects, the playlist ducks out until they
          disconnect.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {live && (
          <div className="mb-4 rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm">
            <span className="font-semibold text-destructive">On air:</span> a DJ
            is broadcasting — the playlist is ducked and will fade back in on
            disconnect.
          </div>
        )}
        {error && <p className="text-sm text-destructive">{error}</p>}
        {streamers === null && !error ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : streamers && streamers.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No streamers yet. Add one to hand out source credentials.
          </p>
        ) : (
          <ul className="divide-y">
            {streamers?.map((s) => (
              <li
                key={s.id}
                className="flex items-center justify-between gap-4 py-3"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{s.name}</span>
                    {!s.enabled && (
                      <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                        disabled
                      </span>
                    )}
                  </div>
                  <p className="truncate text-sm text-muted-foreground">
                    {s.description || s.id}
                  </p>
                </div>
                <div className="flex shrink-0 gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => openConnect(s)}
                  >
                    <RadioTower />
                    Connect
                  </Button>
                  <Button variant="ghost" size="sm" onClick={() => openEdit(s)}>
                    <Pencil />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-destructive hover:text-destructive"
                    onClick={() => remove(s)}
                  >
                    <Trash />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </CardContent>

      {/* Create / edit */}
      <Dialog open={formOpen} onOpenChange={setFormOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>
              {editing ? `Edit ${editing.name}` : "Add streamer"}
            </DialogTitle>
            <DialogDescription>
              Give the DJ their mount URL and source password below.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="st-name">Name</Label>
              <input
                id="st-name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder="DJ Sarah"
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="st-desc">Description</Label>
              <input
                id="st-desc"
                value={form.description}
                onChange={(e) =>
                  setForm({ ...form, description: e.target.value })
                }
                placeholder="Weekend daytime slot"
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="st-pw">
                Source password{" "}
                {editing && (
                  <span className="text-xs text-muted-foreground">
                    (blank keeps the current one)
                  </span>
                )}
              </Label>
              <input
                id="st-pw"
                value={form.source_password}
                onChange={(e) =>
                  setForm({ ...form, source_password: e.target.value })
                }
                placeholder="a-secret-password"
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={form.enabled}
                onChange={(e) =>
                  setForm({ ...form, enabled: e.target.checked })
                }
              />
              Enabled (password accepted by the harbor)
            </label>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setFormOpen(false)}>
              Cancel
            </Button>
            <Button onClick={save}>
              {editing ? "Save changes" : "Add streamer"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Connect instructions */}
      <Dialog
        open={connectInfo !== null}
        onOpenChange={(open) => {
          if (!open) setConnectInfo(null);
        }}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <MicVocal className="size-4" />
              Connect — {connectInfo?.streamer.name}
            </DialogTitle>
            <DialogDescription>
              Point any Icecast source client at the station mount.
            </DialogDescription>
          </DialogHeader>
          {connectInfo && (
            <div className="grid gap-4 text-sm">
              <div className="grid gap-1">
                <span className="text-xs text-muted-foreground">URL</span>
                <code className="rounded bg-muted px-2 py-1">
                  {connectInfo.mount_url}
                </code>
              </div>
              <div className="grid gap-1">
                <span className="text-xs text-muted-foreground">
                  Username / password
                </span>
                <code className="rounded bg-muted px-2 py-1">
                  source / {connectInfo.streamer.source_password}
                </code>
              </div>
              <div className="grid gap-1">
                <span className="flex items-center gap-1 text-xs text-muted-foreground">
                  <Terminal className="size-3" />
                  Mic test (copy-paste)
                </span>
                <pre className="overflow-x-auto rounded bg-muted p-2 text-xs">
                  {connectInfo.curl_mic_test}
                </pre>
              </div>
              <p className="text-xs text-muted-foreground">
                The playlist ducks out while you are connected and fades back in
                when you disconnect. Only one DJ can broadcast at a time.
              </p>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setConnectInfo(null)}>
              Close
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
