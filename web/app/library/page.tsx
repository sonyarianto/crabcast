"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import {
  ChevronDownIcon,
  ChevronUpIcon,
  LayoutGridIcon,
  ListIcon,
  LoaderIcon,
  MusicIcon,
  PencilIcon,
  PlayIcon,
  Radio,
  RefreshCwIcon,
  SearchIcon,
  Trash2Icon,
  UploadIcon,
  XIcon,
} from "lucide-react";

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
  deleteMedia,
  getMedia,
  getMediaConfig,
  getMediaFacets,
  listMedia,
  logout,
  updateMediaTags,
  uploadMedia,
  type MediaFacets,
  type MediaFile,
  type MediaList,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

const PAGE_SIZE = 50;

type View = "table" | "grid";

type Loaded =
  | { state: "loading" }
  | { state: "ok"; data: MediaList; config: { storage_dir: string } }
  | { state: "error"; message: string };

type LibraryQuery = {
  q: string;
  artist: string;
  album: string;
  genre: string;
  sort: string;
  order: "asc" | "desc";
};

const FALLBACK_QUERY: LibraryQuery = {
  q: "",
  artist: "",
  album: "",
  genre: "",
  sort: "created_at",
  order: "desc",
};

export default function LibraryPage() {
  const { meState, refresh } = useMe();
  const [loaded, setLoaded] = useState<Loaded>({ state: "loading" });
  const [query, setQuery] = useState<LibraryQuery>(FALLBACK_QUERY);
  const [offset, setOffset] = useState(0);
  const [view, setView] = useState<View>("table");
  const [facets, setFacets] = useState<MediaFacets>({
    artists: [],
    albums: [],
    genres: [],
  });
  const [dragging, setDragging] = useState(false);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Now playing / preview state.
  const [preview, setPreview] = useState<MediaFile | null>(null);
  const [waveform, setWaveform] = useState<number[] | null>(null);
  const [editing, setEditing] = useState<MediaFile | null>(null);

  const reload = useCallback((q: LibraryQuery, off: number) => {
    listMedia({ ...q, limit: PAGE_SIZE, offset: off })
      .then((data) =>
        getMediaConfig().then((config) =>
          setLoaded({ state: "ok", data, config }),
        ),
      )
      .catch((err: unknown) =>
        setLoaded({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, []);

  useEffect(() => {
    getMediaFacets()
      .then(setFacets)
      .catch(() => {
        // facets are decorative; the list still works without them
      });
  }, []);

  useEffect(() => {
    reload(query, offset);
  }, [query, offset, reload]);

  const setSearch = (value: string) => {
    if (searchTimer.current) clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => {
      setOffset(0);
      setQuery((q) => ({ ...q, q: value }));
    }, 250);
  };

  const setFilter = (key: "artist" | "album" | "genre", value: string) => {
    setOffset(0);
    setQuery((q) => ({ ...q, [key]: value }));
  };

  const toggleSort = (col: string) => {
    setOffset(0);
    setQuery((q) => ({
      ...q,
      sort: col,
      order: q.sort === col && q.order === "asc" ? "desc" : "asc",
    }));
  };

  const sortIndicator = (col: string) => {
    if (query.sort !== col) return null;
    return query.order === "asc" ? (
      <ChevronUpIcon className="inline size-3.5" />
    ) : (
      <ChevronDownIcon className="inline size-3.5" />
    );
  };

  const runUpload = async (files: File[]) => {
    const audio = files.filter(
      (f) =>
        f.type.startsWith("audio/") ||
        /\.(mp3|flac|ogg|opus|m4a|aac|wav|wma|aiff|mp4|m4b)$/i.test(f.name),
    );
    if (audio.length === 0) {
      toast.add({
        title: "No audio files",
        description: "Pick MP3, FLAC, Ogg, M4A, AAC or WAV files.",
        type: "error",
        timeout: 5000,
      });
      return;
    }
    setUploading(true);
    try {
      const results = await uploadMedia(audio);
      const created = results.filter((r) => r.status === "created").length;
      const dupes = results.filter((r) => r.status === "duplicate").length;
      const errors = results.filter((r) => r.status === "error");
      toast.add({
        title: `Uploaded ${created} file${created === 1 ? "" : "s"}`,
        description:
          [
            dupes ? `${dupes} duplicate${dupes === 1 ? "" : "s"} skipped` : "",
            errors.length ? `${errors.length} failed` : "",
          ]
            .filter(Boolean)
            .join(" · ") || undefined,
        type: errors.length ? "error" : "success",
        timeout: 6000,
      });
      for (const e of errors.slice(0, 3)) {
        toast.add({
          title: e.filename,
          description: e.message ?? "upload failed",
          type: "error",
          timeout: 4000,
        });
      }
      setOffset(0);
      reload({ ...query, q: query.q }, 0);
    } catch (err) {
      toast.add({
        title: "Upload failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setUploading(false);
    }
  };

  const play = async (file: MediaFile) => {
    setPreview(file);
    setWaveform(null);
    try {
      const detail = await getMedia(file.id);
      setWaveform(detail.waveform ?? null);
    } catch {
      // waveform is a nicety; playback works without it
    }
  };

  const saveTags = async (input: {
    title: string;
    artist: string;
    album: string;
    genre: string;
  }) => {
    if (!editing) return;
    try {
      await updateMediaTags(editing.id, input);
      toast.add({ title: "Tags saved", type: "success", timeout: 3000 });
      setEditing(null);
      reload(query, offset);
      if (preview?.id === editing.id) setPreview(null);
    } catch (err) {
      toast.add({
        title: "Failed to save tags",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const remove = async (file: MediaFile) => {
    if (!confirm(`Delete "${file.title}"? The file is removed from storage.`))
      return;
    try {
      await deleteMedia(file.id);
      toast.add({ title: "File deleted", type: "success", timeout: 3000 });
      if (preview?.id === file.id) setPreview(null);
      reload(query, offset);
    } catch (err) {
      toast.add({
        title: "Delete failed",
        description: err instanceof Error ? err.message : "Unknown error",
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
            <Button variant="ghost" size="sm" render={<Link href="/users" />}>
              Users
            </Button>
          )}
          <Button variant="ghost" size="sm" render={<Link href="/stations" />}>
            Stations
          </Button>
          <Button variant="ghost" size="sm" render={<Link href="/settings" />}>
            Settings
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

      <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
        <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">
              Media library
            </h1>
            <p className="text-sm text-muted-foreground">
              Upload once, tag it, and point a station&apos;s playlist directory
              at the library to play it on air.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => reload(query, offset)}
            >
              <RefreshCwIcon />
              Refresh
            </Button>
            <Dialog>
              <DialogTrigger render={<Button />}>
                <UploadIcon />
                Upload
              </DialogTrigger>
              <DialogContent className="sm:max-w-md">
                <DialogHeader>
                  <DialogTitle>Upload music</DialogTitle>
                  <DialogDescription>
                    Drop files anywhere on the page, or pick them here.
                    Duplicates are skipped automatically.
                  </DialogDescription>
                </DialogHeader>
                <div className="grid gap-4">
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept="audio/*,.mp3,.flac,.ogg,.opus,.m4a,.aac,.wav,.wma,.aiff"
                    multiple
                    onChange={(e) => {
                      const files = Array.from(e.target.files ?? []);
                      if (files.length) runUpload(files);
                      e.target.value = "";
                    }}
                  />
                  {loaded.state === "ok" && (
                    <p className="rounded-md bg-muted/50 p-3 text-xs text-muted-foreground">
                      Files are stored in{" "}
                      <code className="rounded bg-muted px-1">
                        {loaded.config.storage_dir}
                      </code>
                      . Set a station&apos;s <em>playlist directory</em> to that
                      path to broadcast the library.
                    </p>
                  )}
                </div>
                <DialogFooter>
                  <Button
                    onClick={() => fileInputRef.current?.click()}
                    disabled={uploading}
                  >
                    {uploading ? (
                      <>
                        <LoaderIcon className="animate-spin" />
                        Uploading…
                      </>
                    ) : (
                      "Choose files"
                    )}
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </div>

        {/* Drag & drop zone + toolbar */}
        <div
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragging(false);
            runUpload(Array.from(e.dataTransfer.files));
          }}
          className={`mb-4 rounded-xl border-2 border-dashed p-3 transition-colors ${
            dragging ? "border-primary bg-primary/5" : "border-muted"
          }`}
        >
          {uploading ? (
            <div className="flex items-center justify-center gap-2 py-4 text-sm text-muted-foreground">
              <LoaderIcon className="size-4 animate-spin" />
              Uploading…
            </div>
          ) : (
            <p className="py-1 text-center text-sm text-muted-foreground">
              Drop audio files here to upload (drag &amp; drop)
            </p>
          )}
        </div>

        {/* Search + filters */}
        <div className="mb-4 flex flex-wrap items-center gap-2">
          <div className="relative min-w-52 flex-1">
            <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
              defaultValue={query.q}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search title, artist, album, filename…"
              className="h-9 w-full rounded-md border border-input bg-transparent pr-3 pl-8 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
          </div>
          <select
            value={query.artist}
            onChange={(e) => setFilter("artist", e.target.value)}
            className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none"
          >
            <option value="">All artists</option>
            {facets.artists.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </select>
          <select
            value={query.album}
            onChange={(e) => setFilter("album", e.target.value)}
            className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none"
          >
            <option value="">All albums</option>
            {facets.albums.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </select>
          <select
            value={query.genre}
            onChange={(e) => setFilter("genre", e.target.value)}
            className="h-9 rounded-md border border-input bg-transparent px-2 text-sm shadow-xs outline-none"
          >
            <option value="">All genres</option>
            {facets.genres.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
          <div className="flex rounded-md border border-input">
            <Button
              variant={view === "table" ? "secondary" : "ghost"}
              size="sm"
              className="rounded-none rounded-l-md"
              onClick={() => setView("table")}
            >
              <ListIcon />
            </Button>
            <Button
              variant={view === "grid" ? "secondary" : "ghost"}
              size="sm"
              className="rounded-none rounded-r-md"
              onClick={() => setView("grid")}
            >
              <LayoutGridIcon />
            </Button>
          </div>
        </div>

        {loaded.state === "loading" && (
          <p className="text-sm text-muted-foreground">Loading…</p>
        )}
        {loaded.state === "error" && (
          <p className="text-sm text-destructive">{loaded.message}</p>
        )}
        {loaded.state === "ok" &&
          (loaded.data.items.length === 0 ? (
            <Card>
              <CardHeader>
                <CardTitle>
                  No tracks{query.q ? " match your search" : " yet"}
                </CardTitle>
                <CardDescription>
                  {query.q
                    ? "Try a different search or clear the filters."
                    : "Upload audio files to start building the library."}
                </CardDescription>
              </CardHeader>
            </Card>
          ) : view === "table" ? (
            <TrackTable
              items={loaded.data.items}
              onPlay={play}
              onEdit={setEditing}
              onDelete={remove}
              onSort={toggleSort}
              sortIndicator={sortIndicator}
            />
          ) : (
            <TrackGrid
              items={loaded.data.items}
              onPlay={play}
              onEdit={setEditing}
              onDelete={remove}
            />
          ))}

        {loaded.state === "ok" && loaded.data.total > PAGE_SIZE && (
          <div className="mt-4 flex items-center justify-between text-sm text-muted-foreground">
            <span>
              {offset + 1}–{Math.min(offset + PAGE_SIZE, loaded.data.total)} of{" "}
              {loaded.data.total}
            </span>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                disabled={offset === 0}
                onClick={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
              >
                Previous
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={offset + PAGE_SIZE >= loaded.data.total}
                onClick={() => setOffset((o) => o + PAGE_SIZE)}
              >
                Next
              </Button>
            </div>
          </div>
        )}
      </main>

      {/* Sticky preview player */}
      {preview && (
        <div className="sticky bottom-0 border-t bg-background/95 backdrop-blur">
          <div className="mx-auto flex max-w-6xl items-center gap-4 px-4 py-3">
            <CoverArt file={preview} className="size-12 rounded-md" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium">{preview.title}</p>
              <p className="truncate text-xs text-muted-foreground">
                {[preview.artist, preview.album].filter(Boolean).join(" · ") ||
                  preview.filename}
              </p>
              {waveform && waveform.length > 1 && (
                <div className="mt-1 flex h-8 items-end gap-px">
                  {waveform.map((v, i) => (
                    <div
                      key={i}
                      className="flex-1 rounded-sm bg-primary/50"
                      style={{ height: `${Math.max(4, Math.round(v * 100))}%` }}
                    />
                  ))}
                </div>
              )}
            </div>
            <audio
              controls
              autoPlay
              src={`/api/media/${preview.id}/stream`}
              className="h-9 w-64 max-w-[40vw]"
            />
            <Button variant="ghost" size="sm" onClick={() => setPreview(null)}>
              <XIcon />
            </Button>
          </div>
        </div>
      )}

      {/* Edit dialog */}
      {editing && (
        <EditDialog
          file={editing}
          onClose={() => setEditing(null)}
          onSave={saveTags}
        />
      )}
    </div>
  );
}

function TrackTable({
  items,
  onPlay,
  onEdit,
  onDelete,
  onSort,
  sortIndicator,
}: {
  items: MediaFile[];
  onPlay: (f: MediaFile) => void;
  onEdit: (f: MediaFile) => void;
  onDelete: (f: MediaFile) => void;
  onSort: (col: string) => void;
  sortIndicator: (col: string) => React.ReactNode;
}) {
  return (
    <div className="overflow-hidden rounded-lg border">
      <table className="w-full text-sm">
        <thead className="bg-muted/50">
          <tr className="text-left">
            {(["title", "artist", "album", "genre", "duration"] as const).map(
              (col) => (
                <th key={col} className="px-3 py-2 font-medium">
                  <button
                    className="inline-flex items-center gap-0.5 capitalize hover:text-foreground"
                    onClick={() => onSort(col)}
                  >
                    {col === "duration" ? "Length" : col}
                    {sortIndicator(col)}
                  </button>
                </th>
              ),
            )}
            <th className="px-3 py-2 font-medium">Size</th>
            <th className="px-3 py-2 text-right font-medium">Actions</th>
          </tr>
        </thead>
        <tbody>
          {items.map((file) => (
            <tr key={file.id} className="border-t">
              <td className="px-3 py-2">
                <div className="flex items-center gap-3">
                  <CoverArt file={file} className="size-9 rounded" />
                  <div className="min-w-0">
                    <p className="max-w-60 truncate font-medium">
                      {file.title}
                    </p>
                    <p className="max-w-60 truncate text-xs text-muted-foreground">
                      {file.filename}
                    </p>
                  </div>
                </div>
              </td>
              <td className="max-w-40 truncate px-3 py-2">
                {file.artist || "—"}
              </td>
              <td className="max-w-40 truncate px-3 py-2">
                {file.album || "—"}
              </td>
              <td className="max-w-32 truncate px-3 py-2">
                {file.genre || "—"}
              </td>
              <td className="px-3 py-2 whitespace-nowrap">
                {formatDuration(file.duration_seconds)}
              </td>
              <td className="px-3 py-2 whitespace-nowrap text-muted-foreground">
                {formatBytes(file.size_bytes)}
              </td>
              <td className="px-3 py-2">
                <div className="flex justify-end gap-1">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => onPlay(file)}
                  >
                    <PlayIcon />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => onEdit(file)}
                  >
                    <PencilIcon />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    className="text-destructive hover:text-destructive"
                    onClick={() => onDelete(file)}
                  >
                    <Trash2Icon />
                  </Button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function TrackGrid({
  items,
  onPlay,
  onEdit,
  onDelete,
}: {
  items: MediaFile[];
  onPlay: (f: MediaFile) => void;
  onEdit: (f: MediaFile) => void;
  onDelete: (f: MediaFile) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5">
      {items.map((file) => (
        <Card key={file.id} className="group overflow-hidden">
          <button
            className="relative block aspect-square w-full overflow-hidden bg-muted"
            onClick={() => onPlay(file)}
          >
            <CoverArt file={file} className="size-full" />
            <span className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
              <PlayIcon className="size-8 text-white" />
            </span>
          </button>
          <CardContent className="p-3">
            <p className="truncate text-sm font-medium">{file.title}</p>
            <p className="truncate text-xs text-muted-foreground">
              {file.artist || "Unknown artist"} ·{" "}
              {formatDuration(file.duration_seconds)}
            </p>
            <div className="mt-2 flex gap-1">
              <Button variant="ghost" size="xs" onClick={() => onEdit(file)}>
                <PencilIcon />
                Edit
              </Button>
              <Button
                variant="ghost"
                size="xs"
                className="text-destructive hover:text-destructive"
                onClick={() => onDelete(file)}
              >
                <Trash2Icon />
              </Button>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

/** Cover art with a music-note fallback when the file has none. */
function CoverArt({ file, className }: { file: MediaFile; className: string }) {
  const [broken, setBroken] = useState(false);
  if (!file.has_cover || broken) {
    return (
      <div
        className={`flex items-center justify-center bg-muted text-muted-foreground ${className}`}
      >
        <MusicIcon className="size-1/3" />
      </div>
    );
  }
  return (
    // eslint-disable-next-line @next/next/no-img-element -- authenticated API stream
    <img
      src={`/api/media/${file.id}/cover`}
      alt=""
      onError={() => setBroken(true)}
      className={`object-cover ${className}`}
    />
  );
}

function EditDialog({
  file,
  onClose,
  onSave,
}: {
  file: MediaFile;
  onClose: () => void;
  onSave: (input: {
    title: string;
    artist: string;
    album: string;
    genre: string;
  }) => void;
}) {
  const [form, setForm] = useState({
    title: file.title,
    artist: file.artist,
    album: file.album,
    genre: file.genre,
  });
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit tags</DialogTitle>
          <DialogDescription>
            Changes are written back into the audio file&apos;s tags.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          {(
            [
              ["title", "Title"],
              ["artist", "Artist"],
              ["album", "Album"],
              ["genre", "Genre"],
            ] as const
          ).map(([key, label]) => (
            <div key={key} className="grid gap-2">
              <Label htmlFor={`edit-${key}`}>{label}</Label>
              <input
                id={`edit-${key}`}
                value={form[key]}
                onChange={(e) =>
                  setForm((f) => ({ ...f, [key]: e.target.value }))
                }
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
          ))}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => onSave(form)}>Save tags</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function formatDuration(seconds: number | null): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
