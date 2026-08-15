"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import {
  ActivityIcon,
  ArrowLeftIcon,
  BarChart3Icon,
  BellIcon,
  DownloadIcon,
  HeadphonesIcon,
  Radio,
  UsersIcon,
} from "lucide-react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

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
  getAnalyticsSummary,
  getListenerSeries,
  getRequestStats,
  getStation,
  getTopSongs,
  historyCsvUrl,
  listAlerts,
  logout,
  resolveAlert,
  type Alert,
  type AnalyticsSummary,
  type ListenerPoint,
  type RequestDay,
  type Station,
  type TopSong,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

const SUMMARY_POLL_MS = 30_000;

type Range = "24h" | "7d" | "30d";

const RANGE_BUCKETS: Record<Range, number> = {
  "24h": 30,
  "7d": 60,
  "30d": 360,
};

export default function AnalyticsPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;

  const { meState } = useMe();
  const [station, setStation] = useState<Station | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [range, setRange] = useState<Range>("24h");
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null);
  const [points, setPoints] = useState<ListenerPoint[]>([]);
  const [topSongs, setTopSongs] = useState<TopSong[]>([]);
  const [requestDays, setRequestDays] = useState<RequestDay[]>([]);
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const retryRef = useRef(0);

  const refreshRef = useRef<() => void>(() => {});

  const loadStats = useCallback(() => {
    getAnalyticsSummary(id)
      .then((s) => {
        retryRef.current = 0;
        setSummary(s);
      })
      .catch((err: unknown) => {
        // A station that never had a sample is fine; a dead API is not.
        setError(err instanceof Error ? err.message : "Unknown error");
      });
    listAlerts(id, false)
      .then(setAlerts)
      .catch(() => {
        // alerts are best-effort; don't clobber the page on failure
      });
  }, [id]);

  useEffect(() => {
    refreshRef.current = loadStats;
  }, [loadStats]);

  useEffect(() => {
    let cancelled = false;

    getStation(id)
      .then((s) => {
        if (cancelled) return;
        setStation(s);
        setError(null);
      })
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );

    const interval = setInterval(() => {
      loadStats();
      retryRef.current = Math.min(retryRef.current + 1, 5);
    }, SUMMARY_POLL_MS);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [id, loadStats]);

  // The window is computed in the effect below (Date.now is impure, so it
  // must not run during render); `days` is the pure part needed by the UI.
  const days = range === "24h" ? 1 : range === "7d" ? 7 : 30;

  useEffect(() => {
    let cancelled = false;
    const now = Date.now();
    const toIso = new Date(now).toISOString();
    const fromIso = new Date(now - days * 24 * 3600_000).toISOString();
    getListenerSeries(id, fromIso, toIso, RANGE_BUCKETS[range])
      .then((series) => {
        if (!cancelled) setPoints(series.points);
      })
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : "Unknown error"),
      );
    getTopSongs(id, days)
      .then((songs) => {
        if (!cancelled) setTopSongs(songs);
      })
      .catch(() => {
        // no history yet
      });
    getRequestStats(id, days)
      .then((rows) => {
        if (!cancelled) setRequestDays(rows);
      })
      .catch(() => {
        // no requests yet
      });
    return () => {
      cancelled = true;
    };
  }, [id, days, range]);

  const canManage =
    meState.state === "ready" &&
    (meState.me.user.is_super_admin ||
      meState.me.roles.some(
        (r) =>
          r.role === "station_manager" &&
          (r.station_id === null || r.station_id === id),
      ));

  const resolve = async (alertId: string) => {
    try {
      await resolveAlert(alertId);
      setAlerts((prev) =>
        prev.map((a) =>
          a.id === alertId
            ? { ...a, resolved_at: new Date().toISOString() }
            : a,
        ),
      );
      toast.add({ title: "Alert resolved", type: "success", timeout: 3000 });
    } catch (err) {
      toast.add({
        title: "Could not resolve alert",
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

  if (error && !station) {
    return (
      <Shell {...shellProps}>
        <p className="text-sm text-destructive">{error}</p>
      </Shell>
    );
  }
  if (!station) {
    return (
      <Shell {...shellProps}>
        <p className="text-sm text-muted-foreground">Loading…</p>
      </Shell>
    );
  }

  const openAlerts = alerts.filter((a) => a.resolved_at === null);

  return (
    <Shell {...shellProps}>
      <div className="mb-6 flex flex-wrap items-start justify-between gap-4">
        <div>
          <Link
            href={`/stations/${station.id}`}
            className="mb-2 inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
          >
            <ArrowLeftIcon className="size-4" />
            {station.name}
          </Link>
          <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
            <BarChart3Icon className="size-6" />
            Analytics
          </h1>
          <p className="text-sm text-muted-foreground">
            Listeners, top songs, requests, alerts and uptime.
          </p>
        </div>
        <div className="flex gap-2">
          {(["24h", "7d", "30d"] as const).map((r) => (
            <Button
              key={r}
              variant={range === r ? "default" : "outline"}
              size="sm"
              onClick={() => setRange(r)}
            >
              {r}
            </Button>
          ))}
          <Button
            variant="outline"
            size="sm"
            render={<a href={historyCsvUrl(id, Math.max(days, 30))} />}
          >
            <DownloadIcon />
            Export CSV
          </Button>
        </div>
      </div>

      <div className="mb-4 grid grid-cols-2 gap-3 md:grid-cols-5">
        <StatCard
          icon={<HeadphonesIcon className="size-4" />}
          label="Listeners now"
          value={summary ? String(summary.current_listeners) : "—"}
        />
        <StatCard
          icon={<UsersIcon className="size-4" />}
          label="Unique (24h)"
          value={summary ? String(summary.unique_listeners_24h) : "—"}
        />
        <StatCard
          icon={<ActivityIcon className="size-4" />}
          label="Uptime (24h)"
          value={
            summary?.uptime_percent_24h == null
              ? "—"
              : `${Math.round(summary.uptime_percent_24h)}%`
          }
        />
        <StatCard
          icon={<Radio className="size-4" />}
          label="Plays today"
          value={summary ? String(summary.plays_today) : "—"}
        />
        <StatCard
          icon={<BellIcon className="size-4" />}
          label="Requests today"
          value={summary ? String(summary.requests_today) : "—"}
        />
      </div>

      <Card className="mb-4">
        <CardHeader>
          <CardTitle className="text-base">Listeners</CardTitle>
          <CardDescription>
            Polled from the Icecast admin API; connections is Icecast&apos;s
            cumulative counter (unique listeners ≈ its delta).
          </CardDescription>
        </CardHeader>
        <CardContent>
          {points.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              No listener samples yet — they appear every minute once the
              Icecast admin API responds.
            </p>
          ) : (
            <div className="h-64 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart
                  data={points}
                  margin={{ top: 4, right: 8, left: -12, bottom: 0 }}
                >
                  <defs>
                    <linearGradient
                      id="listenersFill"
                      x1="0"
                      y1="0"
                      x2="0"
                      y2="1"
                    >
                      <stop
                        offset="0%"
                        stopColor="var(--color-primary)"
                        stopOpacity={0.35}
                      />
                      <stop
                        offset="100%"
                        stopColor="var(--color-primary)"
                        stopOpacity={0.02}
                      />
                    </linearGradient>
                  </defs>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    stroke="var(--color-border)"
                  />
                  <XAxis
                    dataKey="ts"
                    tickFormatter={(ts: string) => formatTick(ts, range)}
                    tick={{ fontSize: 11 }}
                    stroke="var(--color-muted-foreground)"
                  />
                  <YAxis
                    allowDecimals={false}
                    width={40}
                    tick={{ fontSize: 11 }}
                    stroke="var(--color-muted-foreground)"
                  />
                  <Tooltip
                    labelFormatter={(ts) =>
                      new Date(String(ts)).toLocaleString()
                    }
                    formatter={(value, name) => [
                      value,
                      name === "listeners" ? "Listeners" : "Connections",
                    ]}
                  />
                  <Area
                    type="monotone"
                    dataKey="listeners"
                    stroke="var(--color-primary)"
                    strokeWidth={2}
                    fill="url(#listenersFill)"
                    name="listeners"
                  />
                  <Line
                    type="monotone"
                    dataKey="connections"
                    stroke="var(--color-muted-foreground)"
                    strokeWidth={1.5}
                    strokeDasharray="4 4"
                    dot={false}
                    name="connections"
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          )}
        </CardContent>
      </Card>

      <div className="mb-4 grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Top songs</CardTitle>
            <CardDescription>
              Most played in the last {days} day(s).
            </CardDescription>
          </CardHeader>
          <CardContent>
            {topSongs.length === 0 ? (
              <p className="text-sm text-muted-foreground">No plays yet.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-8">#</TableHead>
                    <TableHead>Title</TableHead>
                    <TableHead className="text-right">Plays</TableHead>
                    <TableHead className="text-right">Air time</TableHead>
                    <TableHead>Last played</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {topSongs.map((song, i) => (
                    <TableRow key={song.title}>
                      <TableCell className="text-muted-foreground">
                        {i + 1}
                      </TableCell>
                      <TableCell className="max-w-[16rem] truncate">
                        {song.title}
                      </TableCell>
                      <TableCell className="text-right">{song.plays}</TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        {formatDuration(song.total_seconds)}
                      </TableCell>
                      <TableCell className="whitespace-nowrap text-muted-foreground">
                        {new Date(song.last_played_at).toLocaleDateString()}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-base">Requests</CardTitle>
            <CardDescription>
              Accepted, rejected and pending per day.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {requestDays.length === 0 ? (
              <p className="text-sm text-muted-foreground">No requests yet.</p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Day</TableHead>
                    <TableHead className="text-right">Accepted</TableHead>
                    <TableHead className="text-right">Rejected</TableHead>
                    <TableHead className="text-right">Pending</TableHead>
                    <TableHead className="text-right">Total</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {requestDays.map((row) => (
                    <TableRow key={row.day}>
                      <TableCell className="whitespace-nowrap">
                        {row.day}
                      </TableCell>
                      <TableCell className="text-right">
                        {row.accepted}
                      </TableCell>
                      <TableCell className="text-right">
                        {row.rejected}
                      </TableCell>
                      <TableCell className="text-right">
                        {row.pending}
                      </TableCell>
                      <TableCell className="text-right font-medium">
                        {row.total}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Alerts</CardTitle>
          <CardDescription>
            Dead air, engine crash loops, Icecast unreachable and disk space —
            auto-resolved when conditions clear.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {alerts.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No alerts. {openAlerts.length === 0 && "All quiet."}
            </p>
          ) : (
            <div className="space-y-2">
              {alerts.map((alert) => (
                <div
                  key={alert.id}
                  className={`flex items-start justify-between gap-3 rounded-lg border p-3 ${
                    alert.resolved_at === null
                      ? "border-destructive/40"
                      : "border-border opacity-70"
                  }`}
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span
                        className={`rounded-full px-2 py-0.5 text-xs font-semibold ${
                          alert.severity === "error"
                            ? "text-destructive-foreground bg-destructive"
                            : "bg-muted text-muted-foreground"
                        }`}
                      >
                        {alert.severity}
                      </span>
                      <span className="text-sm font-medium">{alert.title}</span>
                      <span className="text-xs text-muted-foreground">
                        {alert.resolved_at === null ? "open" : "resolved"}
                      </span>
                    </div>
                    {alert.detail && (
                      <p className="mt-1 truncate text-xs text-muted-foreground">
                        {alert.detail}
                      </p>
                    )}
                    <p className="mt-1 text-xs text-muted-foreground">
                      {alert.kind} ·{" "}
                      {new Date(alert.created_at).toLocaleString()}
                      {alert.resolved_at &&
                        ` · resolved ${new Date(alert.resolved_at).toLocaleString()}`}
                    </p>
                  </div>
                  {alert.resolved_at === null && canManage && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => resolve(alert.id)}
                    >
                      Resolve
                    </Button>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </Shell>
  );
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-lg border p-3">
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        {icon}
        {label}
      </div>
      <div className="mt-1 text-xl font-semibold">{value}</div>
    </div>
  );
}

function formatTick(ts: string, range: Range): string {
  const date = new Date(ts);
  if (range === "24h") {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function formatDuration(seconds: number): string {
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0)
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function Shell({
  children,
  me,
  onLogout,
}: {
  children: React.ReactNode;
  me?: { displayName: string; isSuperAdmin: boolean } | null;
  onLogout?: () => void;
}) {
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          {me?.isSuperAdmin && (
            <Button variant="ghost" size="sm" render={<Link href="/users" />}>
              Users
            </Button>
          )}
          <Button variant="ghost" size="sm" render={<Link href="/library" />}>
            Library
          </Button>
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
      <main className="mx-auto w-full max-w-5xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  );
}
