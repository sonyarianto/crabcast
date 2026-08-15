"use client";

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import { useParams } from "react-router";
import {
  ArrowLeftIcon,
  Mic2Icon,
  PlusIcon,
  Radio,
  RssIcon,
  SearchIcon,
  Trash2Icon,
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
  createPodcastEpisode,
  deletePodcastEpisode,
  listMedia,
  listPodcasts,
  logout,
  podcastRssUrl,
  type MediaFile,
  type PodcastEpisode,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

type Status =
  | { state: "loading" }
  | { state: "ok"; episodes: PodcastEpisode[] }
  | { state: "error"; message: string };

export default function PodcastsPage() {
  const params = useParams<{ id: string }>();
  const stationId = params.id!;
  const { meState } = useMe();
  const [status, setStatus] = useState<Status>({ state: "loading" });

  const reload = useCallback(() => {
    listPodcasts(stationId)
      .then((episodes) => setStatus({ state: "ok", episodes }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, [stationId]);

  useEffect(() => {
    listPodcasts(stationId)
      .then((episodes) => setStatus({ state: "ok", episodes }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, [stationId]);

  const remove = async (episode: PodcastEpisode) => {
    try {
      await deletePodcastEpisode(episode.id);
      toast.add({ title: "Episode deleted", type: "success", timeout: 3000 });
      reload();
    } catch (err) {
      toast.add({
        title: "Could not delete episode",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const shellProps = {
    me:
      meState.state === "ready"
        ? {
            displayName:
              meState.me.user.display_name || meState.me.user.username,
            isSuperAdmin: meState.me.user.is_super_admin,
          }
        : null,
    onLogout: async () => {
      await logout();
      window.location.reload();
    },
  } as const;

  return (
    <Shell {...shellProps} stationId={stationId}>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
            <Mic2Icon className="size-6" />
            Podcasts
          </h1>
          <p className="text-sm text-muted-foreground">
            Publish audio as episodes with an RSS feed any podcast app can
            subscribe to.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            render={<a href={podcastRssUrl(stationId)} target="_blank" />}
          >
            <RssIcon />
            RSS feed
          </Button>
          <CreateEpisodeDialog
            stationId={stationId}
            onCreated={() => {
              reload();
              toast.add({
                title: "Episode published",
                type: "success",
                timeout: 3000,
              });
            }}
          />
        </div>
      </div>

      {status.state === "loading" && (
        <p className="text-sm text-muted-foreground">Loading…</p>
      )}
      {status.state === "error" && (
        <p className="text-sm text-destructive">{status.message}</p>
      )}
      {status.state === "ok" && status.episodes.length === 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">No episodes yet</CardTitle>
            <CardDescription>
              Publish your first episode — pick an audio file from the library
              and give it a title.
            </CardDescription>
          </CardHeader>
        </Card>
      )}
      {status.state === "ok" &&
        status.episodes.map((ep) => (
          <Card key={ep.id} className="mb-3">
            <CardContent className="flex items-center justify-between gap-4 py-4">
              <div className="min-w-0">
                <p className="truncate font-medium">{ep.title}</p>
                {ep.description && (
                  <p className="truncate text-sm text-muted-foreground">
                    {ep.description}
                  </p>
                )}
                <p className="mt-0.5 text-xs text-muted-foreground">
                  Published {new Date(ep.created_at).toLocaleString()}
                </p>
              </div>
              <Button variant="ghost" size="sm" onClick={() => void remove(ep)}>
                <Trash2Icon />
                Delete
              </Button>
            </CardContent>
          </Card>
        ))}
    </Shell>
  );
}

function CreateEpisodeDialog({
  stationId,
  onCreated,
}: {
  stationId: string;
  onCreated: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [q, setQ] = useState("");
  const [results, setResults] = useState<MediaFile[]>([]);
  const [picked, setPicked] = useState<MediaFile | null>(null);
  const [creating, setCreating] = useState(false);

  const search = async (query: string) => {
    setQ(query);
    if (!query.trim()) {
      setResults([]);
      setPicked(null);
      return;
    }
    const data = await listMedia({ q: query, limit: 20, sort: "title" });
    setResults(data.items);
  };

  const create = async () => {
    if (!picked || !title.trim()) return;
    setCreating(true);
    try {
      await createPodcastEpisode(stationId, {
        media_id: picked.id,
        title: title.trim(),
        description,
      });
      setOpen(false);
      setTitle("");
      setDescription("");
      setQ("");
      setResults([]);
      setPicked(null);
      onCreated();
    } catch (err) {
      toast.add({
        title: "Could not publish episode",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setCreating(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button size="sm" />}>
        <PlusIcon />
        New episode
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Publish an episode</DialogTitle>
          <DialogDescription>
            Pick an audio file from the library; it becomes the episode&apos;s
            audio.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="ep-title">Title</Label>
            <input
              id="ep-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Episode 1 — the launch mix"
              className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="ep-desc">Description (optional)</Label>
            <input
              id="ep-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What this episode is about"
              className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
            />
          </div>
          <div className="space-y-1.5">
            <Label>Audio file</Label>
            <div className="relative">
              <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
              <input
                value={q}
                onChange={(e) => void search(e.target.value)}
                placeholder="Search the library…"
                className="h-9 w-full rounded-md border border-input bg-transparent pr-3 pl-8 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
            {picked && (
              <p className="flex items-center gap-2 rounded-md bg-muted px-3 py-2 text-sm">
                <Mic2Icon className="size-4" />
                <span className="truncate">
                  {picked.title || picked.filename}
                </span>
              </p>
            )}
            {results.length > 0 && !picked && (
              <div className="max-h-48 space-y-1 overflow-auto">
                {results.map((f) => (
                  <button
                    key={f.id}
                    onClick={() => {
                      setPicked(f);
                      setResults([]);
                    }}
                    className="flex w-full items-center gap-3 rounded-md p-2 text-left text-sm hover:bg-muted"
                  >
                    <span className="truncate">{f.title || f.filename}</span>
                    <span className="ml-auto shrink-0 text-xs text-muted-foreground">
                      {f.artist || "unknown artist"}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button
            onClick={create}
            disabled={creating || !picked || !title.trim()}
          >
            {creating ? "Publishing…" : "Publish"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Shell({
  children,
  me,
  onLogout,
  stationId,
}: {
  children: React.ReactNode;
  me?: { displayName: string; isSuperAdmin: boolean } | null;
  onLogout?: () => void;
  stationId: string;
}) {
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="sm" render={<Link to="/stations" />}>
            Stations
          </Button>
          {me?.isSuperAdmin && (
            <Button variant="ghost" size="sm" render={<Link to="/users" />}>
              Users
            </Button>
          )}
          {me && (
            <>
              <span className="text-sm text-muted-foreground">
                {me.displayName}
              </span>
              <Button variant="ghost" size="sm" onClick={onLogout}>
                Log out
              </Button>
            </>
          )}
          <ThemeToggle />
        </div>
      </header>
      <main className="mx-auto w-full max-w-2xl flex-1 px-4 py-8">
        <Button
          variant="ghost"
          size="sm"
          className="mb-4"
          render={<Link to={`/stations/${stationId}`} />}
        >
          <ArrowLeftIcon />
          Back to station
        </Button>
        {children}
      </main>
    </div>
  );
}
