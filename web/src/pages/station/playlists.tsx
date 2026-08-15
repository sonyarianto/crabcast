"use client";

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import { useParams } from "react-router";
import {
  ArrowLeftIcon,
  ChevronDownIcon,
  ChevronUpIcon,
  ClockIcon,
  PencilIcon,
  PlusIcon,
  Radio,
  SearchIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { LanguageToggle } from "@/components/language-toggle";
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
  addPlaylistSchedule,
  addPlaylistTracks,
  createPlaylist,
  deletePlaylist,
  deletePlaylistSchedule,
  getPlaylistPreview,
  listMedia,
  listPlaylists,
  logout,
  removePlaylistTrack,
  reorderPlaylistTracks,
  updatePlaylist,
  updatePlaylistTrackOverrides,
  type MediaFile,
  type PlaylistDetail,
  type PlaylistInput,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

const KIND_KEYS: Record<PlaylistInput["kind"], string> = {
  standard: "playlists.kind_standard",
  looping: "playlists.kind_looping",
  scheduled: "playlists.kind_scheduled",
  once_per_hour: "playlists.kind_once_per_hour",
};

export default function PlaylistsPage() {
  const { t } = useTranslation();
  const params = useParams<{ id: string }>();
  const stationId = params.id!;

  const { meState, refresh } = useMe();
  const [playlists, setPlaylists] = useState<PlaylistDetail[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);

  const reload = useCallback(() => {
    listPlaylists(stationId)
      .then(setPlaylists)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : t("common.unknown_error")),
      );
  }, [stationId, t]);

  useEffect(reload, [reload]);

  const togglePreview = async () => {
    if (preview !== null) {
      setPreview(null);
      return;
    }
    try {
      const p = await getPlaylistPreview(stationId);
      setPreview(p.lua);
    } catch (err) {
      toast.add({
        title: t("playlists.preview_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const isAdmin = meState.state === "ready" && meState.me.user.is_super_admin;

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          {isAdmin && (
            <Button variant="ghost" size="sm" render={<Link to="/users" />}>
              {t("nav.users")}
            </Button>
          )}
          <Button variant="ghost" size="sm" render={<Link to="/stations" />}>
            {t("nav.stations")}
          </Button>
          <Button variant="ghost" size="sm" render={<Link to="/library" />}>
            {t("nav.library")}
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
                {t("nav.logout")}
              </Button>
            </>
          )}
          <LanguageToggle />
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-8">
        <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
          <div>
            <Link
              to={`/stations/${stationId}`}
              className="mb-2 inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
            >
              <ArrowLeftIcon className="size-4" />
              {t("playlists.back_link")}
            </Link>
            <h1 className="text-2xl font-semibold tracking-tight">
              {t("playlists.title")}
            </h1>
            <p className="text-sm text-muted-foreground">{t("playlists.subtitle")}</p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={togglePreview}>
              {preview !== null ? t("playlists.hide_lua") : t("playlists.preview_lua")}
            </Button>
            <CreatePlaylistDialog stationId={stationId} onCreated={reload} />
          </div>
        </div>

        {preview !== null && (
          <Card className="mb-6">
            <CardHeader>
              <CardTitle className="text-base">{t("playlists.preview_title")}</CardTitle>
              <CardDescription>{t("playlists.preview_desc")}</CardDescription>
            </CardHeader>
            <CardContent>
              <pre className="max-h-96 overflow-auto rounded-lg bg-muted/50 p-4 text-xs leading-relaxed">
                {preview}
              </pre>
            </CardContent>
          </Card>
        )}

        {error && <p className="mb-4 text-sm text-destructive">{error}</p>}

        {playlists === null && !error && (
          <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
        )}

        {playlists !== null && playlists.length === 0 && (
          <Card>
            <CardHeader>
              <CardTitle>{t("playlists.no_playlists")}</CardTitle>
              <CardDescription>{t("playlists.no_playlists_desc")}</CardDescription>
            </CardHeader>
          </Card>
        )}

        {playlists?.map((pl) => (
          <PlaylistCard key={pl.id} playlist={pl} onChange={reload} />
        ))}
      </main>
    </div>
  );
}

function CreatePlaylistDialog({
  stationId,
  onCreated,
}: {
  stationId: string;
  onCreated: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState<PlaylistInput>({
    name: "",
    kind: "standard",
    weight: 1,
    shuffle: false,
    enabled: true,
  });
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    try {
      await createPlaylist(stationId, form);
      toast.add({ title: t("playlists.created"), type: "success", timeout: 3000 });
      setOpen(false);
      setForm({
        name: "",
        kind: "standard",
        weight: 1,
        shuffle: false,
        enabled: true,
      });
      onCreated();
    } catch (err) {
      toast.add({
        title: t("playlists.create_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button />}>
        <PlusIcon />
        {t("playlists.new_playlist")}
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("playlists.new_playlist")}</DialogTitle>
          <DialogDescription>{t("playlists.new_playlist_desc")}</DialogDescription>
        </DialogHeader>
        <PlaylistForm form={form} setForm={setForm} />
        <DialogFooter>
          <Button onClick={save} disabled={saving || !form.name.trim()}>
            {saving ? t("playlists.creating") : t("playlists.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PlaylistForm({
  form,
  setForm,
}: {
  form: PlaylistInput;
  setForm: (f: PlaylistInput) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="grid gap-4">
      <div className="grid gap-2">
        <Label htmlFor="pl-name">{t("playlists.field_name")}</Label>
        <input
          id="pl-name"
          value={form.name}
          onChange={(e) => setForm({ ...form, name: e.target.value })}
          placeholder={t("playlists.name_placeholder")}
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        />
      </div>
      <div className="grid gap-2">
        <Label htmlFor="pl-kind">{t("playlists.field_type")}</Label>
        <select
          id="pl-kind"
          value={form.kind}
          onChange={(e) =>
            setForm({ ...form, kind: e.target.value as PlaylistInput["kind"] })
          }
          className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none"
        >
          {(Object.keys(KIND_KEYS) as PlaylistInput["kind"][]).map((k) => (
            <option key={k} value={k}>
              {t(KIND_KEYS[k])}
            </option>
          ))}
        </select>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <div className="grid gap-2">
          <Label htmlFor="pl-weight">{t("playlists.field_weight")}</Label>
          <input
            id="pl-weight"
            type="number"
            min={1}
            value={form.weight}
            onChange={(e) =>
              setForm({ ...form, weight: Number(e.target.value) })
            }
            className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
        </div>
        <div className="flex items-end gap-4 pb-1">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.shuffle}
              onChange={(e) => setForm({ ...form, shuffle: e.target.checked })}
              disabled={form.kind === "looping"}
            />
            {t("playlists.shuffle")}
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
            />
            {t("playlists.enabled")}
          </label>
        </div>
      </div>
    </div>
  );
}

function PlaylistCard({
  playlist,
  onChange,
}: {
  playlist: PlaylistDetail;
  onChange: () => void;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [addOpen, setAddOpen] = useState(false);

  const run = async (fn: () => Promise<unknown>, okMsg: string) => {
    try {
      await fn();
      toast.add({ title: okMsg, type: "success", timeout: 3000 });
      onChange();
    } catch (err) {
      toast.add({
        title: t("playlists.operation_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const move = async (index: number, dir: -1 | 1) => {
    const order = playlist.tracks.map((t) => t.media_id);
    const target = index + dir;
    if (target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    await run(() => reorderPlaylistTracks(playlist.id, order), t("playlists.order_updated"));
  };

  const remove = async (mediaId: string) => {
    if (!confirm(t("playlists.confirm_remove_track", { name: playlist.name }))) return;
    await run(() => removePlaylistTrack(playlist.id, mediaId), t("playlists.track_removed"));
  };

  const pl = playlist;

  return (
    <Card className="mb-4">
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div>
            <CardTitle className="flex items-center gap-2 text-base">
              {pl.name}
              <span className="rounded-full bg-muted px-2 py-0.5 text-xs font-normal text-muted-foreground">
                {t(KIND_KEYS[pl.kind])}
              </span>
              {!pl.enabled && (
                <span className="rounded-full bg-muted px-2 py-0.5 text-xs font-normal text-muted-foreground">
                  {t("common.disabled")}
                </span>
              )}
            </CardTitle>
            <CardDescription>
              {t("playlists.meta_tracks", {
                count: pl.tracks.length,
                weight: pl.weight,
              })}
              {pl.kind === "scheduled" &&
                t("playlists.meta_schedules", { count: pl.schedules.length })}
            </CardDescription>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setAddOpen(true)}
            >
              <PlusIcon />
              {t("playlists.add_tracks")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setEditing(true)}
            >
              <PencilIcon />
              {t("playlists.edit")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => {
                if (confirm(t("playlists.confirm_delete", { name: pl.name })))
                  run(() => deletePlaylist(pl.id), t("playlists.deleted"));
              }}
            >
              <Trash2Icon />
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {pl.tracks.length === 0 ? (
          <p className="text-sm text-muted-foreground">{t("playlists.no_tracks")}</p>
        ) : (
          <div className="overflow-hidden rounded-lg border">
            <table className="w-full text-sm">
              <thead className="bg-muted/50">
                <tr className="text-left">
                  <th className="px-3 py-2 font-medium">{t("playlists.col_hash")}</th>
                  <th className="px-3 py-2 font-medium">{t("playlists.col_track")}</th>
                  <th className="px-3 py-2 font-medium">{t("playlists.col_overrides")}</th>
                  <th className="px-3 py-2 text-right font-medium">{t("playlists.col_actions")}</th>
                </tr>
              </thead>
              <tbody>
                {pl.tracks.map((track, i) => (
                  <TrackRow
                    key={track.media_id}
                    playlistId={pl.id}
                    index={i}
                    total={pl.tracks.length}
                    track={track}
                    onMove={(dir) => move(i, dir)}
                    onRemove={() => remove(track.media_id)}
                    onSaved={() =>
                      run(() => Promise.resolve(), t("playlists.overrides_saved"))
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}

        {pl.kind === "scheduled" && (
          <SchedulesSection playlist={pl} onChange={onChange} />
        )}
      </CardContent>

      <EditPlaylistDialog
        key={pl.id}
        playlist={pl}
        open={editing}
        onOpenChange={setEditing}
        onSaved={onChange}
      />
      <AddTracksDialog
        key={pl.id}
        playlist={pl}
        open={addOpen}
        onOpenChange={setAddOpen}
        onAdded={onChange}
      />
    </Card>
  );
}

function TrackRow({
  playlistId,
  index,
  total,
  track,
  onMove,
  onRemove,
  onSaved,
}: {
  playlistId: string;
  index: number;
  total: number;
  track: PlaylistDetail["tracks"][number];
  onMove: (dir: -1 | 1) => void;
  onRemove: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const [overrides, setOverrides] = useState({
    fade_in: track.fade_in ?? "",
    fade_out: track.fade_out ?? "",
    cue_in: track.cue_in ?? "",
    cue_out: track.cue_out ?? "",
  });

  const save = async () => {
    const num = (v: string | number) =>
      v === "" || v === null ? null : Number(v);
    await updatePlaylistTrackOverrides(playlistId, track.media_id, {
      fade_in: num(overrides.fade_in),
      fade_out: num(overrides.fade_out),
      cue_in: num(overrides.cue_in),
      cue_out: num(overrides.cue_out),
    });
    onSaved();
  };

  const inputClass =
    "h-7 w-14 rounded border border-input bg-transparent px-1 text-xs";

  return (
    <tr className="border-t">
      <td className="px-3 py-2 text-muted-foreground">{index + 1}</td>
      <td className="max-w-56 truncate px-3 py-2 font-medium">
        {track.media_id}
      </td>
      <td className="px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">{t("playlists.fade_in")}</span>
          <input
            value={String(overrides.fade_in)}
            onChange={(e) =>
              setOverrides({ ...overrides, fade_in: e.target.value })
            }
            onBlur={save}
            placeholder="—"
            className={inputClass}
          />
          <span className="text-xs text-muted-foreground">{t("playlists.fade_out")}</span>
          <input
            value={String(overrides.fade_out)}
            onChange={(e) =>
              setOverrides({ ...overrides, fade_out: e.target.value })
            }
            onBlur={save}
            placeholder="—"
            className={inputClass}
          />
          <span className="text-xs text-muted-foreground">{t("playlists.cue_in")}</span>
          <input
            value={String(overrides.cue_in)}
            onChange={(e) =>
              setOverrides({ ...overrides, cue_in: e.target.value })
            }
            onBlur={save}
            placeholder="—"
            className={inputClass}
          />
          <span className="text-xs text-muted-foreground">{t("playlists.cue_out")}</span>
          <input
            value={String(overrides.cue_out)}
            onChange={(e) =>
              setOverrides({ ...overrides, cue_out: e.target.value })
            }
            onBlur={save}
            placeholder="—"
            className={inputClass}
          />
        </div>
      </td>
      <td className="px-3 py-2">
        <div className="flex justify-end gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={index === 0}
            onClick={() => onMove(-1)}
          >
            <ChevronUpIcon />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={index === total - 1}
            onClick={() => onMove(1)}
          >
            <ChevronDownIcon />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={onRemove}>
            <XIcon />
          </Button>
        </div>
      </td>
    </tr>
  );
}

function SchedulesSection({
  playlist,
  onChange,
}: {
  playlist: PlaylistDetail;
  onChange: () => void;
}) {
  const { t } = useTranslation();
  const [days, setDays] = useState<string[]>([]);
  const [start, setStart] = useState("09:00");
  const [end, setEnd] = useState("17:00");

  const add = async () => {
    if (!start || !end) return;
    try {
      await addPlaylistSchedule(playlist.id, {
        days: days.join(","),
        start_time: start,
        end_time: end,
      });
      toast.add({ title: t("playlists.schedule_added"), type: "success", timeout: 3000 });
      onChange();
    } catch (err) {
      toast.add({
        title: t("playlists.schedule_add_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const remove = async (scheduleId: string) => {
    await deletePlaylistSchedule(playlist.id, scheduleId);
    toast.add({ title: t("playlists.schedule_removed"), type: "success", timeout: 3000 });
    onChange();
  };

  return (
    <div className="rounded-lg border p-3">
      <p className="mb-2 flex items-center gap-1.5 text-sm font-medium">
        <ClockIcon className="size-4" />
        {t("playlists.daypart_rules")}
      </p>
      {playlist.schedules.length > 0 && (
        <ul className="mb-3 space-y-1 text-sm">
          {playlist.schedules.map((s) => (
            <li key={s.id} className="flex items-center justify-between gap-2">
              <span>
                <span className="font-medium">
                  {s.days ? s.days.toUpperCase() : t("playlists.every_day")}
                </span>{" "}
                · {s.start_time}–{s.end_time}
              </span>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => remove(s.id)}
              >
                <XIcon />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex flex-wrap gap-1.5">
          {DAYS.map((d) => (
            <label
              key={d}
              className={`flex h-7 cursor-pointer items-center rounded border px-2 text-xs capitalize ${
                days.includes(d)
                  ? "border-primary bg-primary/10"
                  : "border-input"
              }`}
            >
              <input
                type="checkbox"
                className="sr-only"
                checked={days.includes(d)}
                onChange={(e) =>
                  setDays((prev) =>
                    e.target.checked
                      ? [...prev, d]
                      : prev.filter((x) => x !== d),
                  )
                }
              />
              {d}
            </label>
          ))}
        </div>
        <input
          type="time"
          value={start}
          onChange={(e) => setStart(e.target.value)}
          className="h-9 rounded-md border border-input bg-transparent px-2 text-sm"
        />
        <span className="text-xs text-muted-foreground">{t("playlists.to")}</span>
        <input
          type="time"
          value={end}
          onChange={(e) => setEnd(e.target.value)}
          className="h-9 rounded-md border border-input bg-transparent px-2 text-sm"
        />
        <Button variant="outline" size="sm" onClick={add}>
          <PlusIcon />
          {t("playlists.add_rule")}
        </Button>
      </div>
    </div>
  );
}

function EditPlaylistDialog({
  playlist,
  open,
  onOpenChange,
  onSaved,
}: {
  playlist: PlaylistDetail;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
}) {
  // Keyed by playlist id at the call site, so state initializes fresh per
  // playlist (no prop-sync effect needed).
  const { t } = useTranslation();
  const [form, setForm] = useState<PlaylistInput>({
    name: playlist.name,
    kind: playlist.kind,
    weight: playlist.weight,
    shuffle: playlist.shuffle,
    enabled: playlist.enabled,
  });
  const [saving, setSaving] = useState(false);

  const save = async () => {
    setSaving(true);
    try {
      await updatePlaylist(playlist.id, form);
      toast.add({ title: t("playlists.updated"), type: "success", timeout: 3000 });
      onOpenChange(false);
      onSaved();
    } catch (err) {
      toast.add({
        title: t("playlists.update_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("playlists.edit_title")}</DialogTitle>
          <DialogDescription>{t("playlists.edit_desc")}</DialogDescription>
        </DialogHeader>
        <PlaylistForm form={form} setForm={setForm} />
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={save} disabled={saving || !form.name.trim()}>
            {saving ? t("playlists.saving") : t("playlists.save_playlist")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function AddTracksDialog({
  playlist,
  open,
  onOpenChange,
  onAdded,
}: {
  playlist: PlaylistDetail;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: () => void;
}) {
  const { t } = useTranslation();
  const [q, setQ] = useState("");
  const [results, setResults] = useState<MediaFile[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [adding, setAdding] = useState(false);
  const [searched, setSearched] = useState(false);

  const search = async (query: string) => {
    setQ(query);
    if (!query.trim()) {
      setResults([]);
      setSearched(false);
      return;
    }
    const data = await listMedia({ q: query, limit: 20, sort: "title" });
    setResults(data.items);
    setSearched(true);
  };

  const add = async () => {
    setAdding(true);
    try {
      const { added } = await addPlaylistTracks(playlist.id, [...selected]);
      toast.add({
        title: t("playlists.added_tracks", { count: added }),
        type: "success",
        timeout: 3000,
      });
      setSelected(new Set());
      onOpenChange(false);
      onAdded();
    } catch (err) {
      toast.add({
        title: t("playlists.add_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setAdding(false);
    }
  };

  const already = new Set(playlist.tracks.map((t) => t.media_id));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("playlists.add_tracks_title", { name: playlist.name })}</DialogTitle>
          <DialogDescription>{t("playlists.add_tracks_desc")}</DialogDescription>
        </DialogHeader>
        <div className="relative">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <input
            value={q}
            onChange={(e) => search(e.target.value)}
            placeholder={t("playlists.search_placeholder")}
            className="h-9 w-full rounded-md border border-input bg-transparent pr-3 pl-8 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
          />
        </div>
        {results.length > 0 && (
          <div className="max-h-64 space-y-1 overflow-auto">
            {results.map((f) => {
              const inPlaylist = already.has(f.id);
              return (
                <label
                  key={f.id}
                  className={`flex items-center gap-3 rounded-md p-2 text-sm ${
                    inPlaylist ? "opacity-50" : "hover:bg-muted"
                  }`}
                >
                  <input
                    type="checkbox"
                    className="size-4"
                    disabled={inPlaylist}
                    checked={inPlaylist || selected.has(f.id)}
                    onChange={(e) =>
                      setSelected((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) next.add(f.id);
                        else next.delete(f.id);
                        return next;
                      })
                    }
                  />
                  <span className="min-w-0">
                    <span className="block truncate font-medium">
                      {f.title}
                    </span>
                    <span className="block truncate text-xs text-muted-foreground">
                      {f.artist || t("playlists.unknown_artist")}
                      {f.duration_seconds
                        ? ` · ${Math.floor(f.duration_seconds / 60)}:${String(
                            Math.floor(f.duration_seconds % 60),
                          ).padStart(2, "0")}`
                        : ""}
                    </span>
                  </span>
                </label>
              );
            })}
          </div>
        )}
        {searched && results.length === 0 && (
          <p className="text-sm text-muted-foreground">{t("playlists.no_matches")}</p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={add} disabled={adding || selected.size === 0}>
            {adding
              ? t("playlists.adding")
              : t("playlists.add_tracks_btn", { count: selected.size })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
