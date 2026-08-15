"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { Radio, UsersIcon } from "lucide-react";

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
  createUser,
  deleteUser,
  listAudit,
  listStations,
  listUsers,
  updateUser,
  type AuditEntry,
  type RoleGrant,
  type Station,
  type UserWithRoles,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

type Status =
  | { state: "loading" }
  | { state: "ok"; users: UserWithRoles[]; audit: AuditEntry[] }
  | { state: "error"; message: string };

const ROLES = [
  {
    name: "station_manager",
    label: "Station manager",
    hint: "manage stations",
  },
  { name: "dj", label: "DJ", hint: "live control: skip, jingles" },
  { name: "media_editor", label: "Media editor", hint: "edit media (Phase 4)" },
] as const;

export default function UsersPage() {
  const { meState } = useMe();
  const [status, setStatus] = useState<Status>({ state: "loading" });
  const [stations, setStations] = useState<Station[]>([]);
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [isSuperAdmin, setIsSuperAdmin] = useState(false);
  const [grants, setGrants] = useState<RoleGrant[]>([]);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(() => {
    setStatus({ state: "loading" });
    Promise.all([listUsers(), listAudit()])
      .then(([users, audit]) => setStatus({ state: "ok", users, audit }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
  }, []);

  useEffect(() => {
    Promise.all([listUsers(), listAudit()])
      .then(([users, audit]) => setStatus({ state: "ok", users, audit }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : "Unknown error",
        }),
      );
    listStations()
      .then(setStations)
      .catch(() => setStations([]));
  }, []);

  const canAdmin = meState.state === "ready" && meState.me.user.is_super_admin;

  const toggleGrant = (role: string, stationId: string | null) => {
    setGrants((g) => {
      const key = `${role}:${stationId ?? "*"}`;
      const existing = g.some(
        (x) => x.role === role && x.station_id === stationId,
      );
      return existing
        ? g.filter((x) => `${x.role}:${x.station_id ?? "*"}` !== key)
        : [...g, { role, station_id: stationId }];
    });
  };

  const submit = async () => {
    setSaving(true);
    try {
      await createUser({
        username,
        password,
        display_name: displayName,
        is_super_admin: isSuperAdmin,
        roles: grants,
      });
      toast.add({ title: "User created", type: "success", timeout: 3000 });
      setUsername("");
      setDisplayName("");
      setPassword("");
      setIsSuperAdmin(false);
      setGrants([]);
      reload();
    } catch (err) {
      toast.add({
        title: "Failed to create user",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const remove = async (user: UserWithRoles) => {
    if (!confirm(`Delete user "${user.username}"?`)) return;
    try {
      await deleteUser(user.id);
      toast.add({ title: "User deleted", type: "success", timeout: 3000 });
      reload();
    } catch (err) {
      toast.add({
        title: "Failed to delete user",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  const toggleSuper = async (user: UserWithRoles) => {
    try {
      await updateUser(user.id, {
        username: user.username,
        display_name: user.display_name,
        is_super_admin: !user.is_super_admin,
        roles: user.roles,
      });
      reload();
    } catch (err) {
      toast.add({
        title: "Failed to update user",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    }
  };

  if (meState.state === "loading") {
    return (
      <div className="flex flex-1 flex-col items-center justify-center">
        <p className="text-sm text-muted-foreground">Loading…</p>
      </div>
    );
  }

  if (!canAdmin) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center">
        <p className="text-sm text-destructive">Super admin required.</p>
        <Link href="/stations" className="mt-2 text-sm underline">
          Back to stations
        </Link>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="sm" render={<Link href="/settings" />}>
            Settings
          </Button>
          <span className="text-sm text-muted-foreground">
            {meState.state === "ready" && meState.me.user.display_name}
          </span>
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-8">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">Users</h1>
            <p className="text-sm text-muted-foreground">
              Accounts and role grants. Roles apply globally or per station.
            </p>
          </div>
          <Dialog>
            <DialogTrigger render={<Button />}>
              <UsersIcon />
              New user
            </DialogTrigger>
            <DialogContent className="sm:max-w-lg">
              <DialogHeader>
                <DialogTitle>New user</DialogTitle>
                <DialogDescription>
                  Set an initial password; the user can change it later.
                </DialogDescription>
              </DialogHeader>
              <div className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="username">Username</Label>
                  <input
                    id="username"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="display_name">Display name</Label>
                  <input
                    id="display_name"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="password">Password</Label>
                  <input
                    id="password"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={isSuperAdmin}
                    onChange={(e) => setIsSuperAdmin(e.target.checked)}
                    className="size-4"
                  />
                  Super admin (full access)
                </label>
                <div className="grid gap-2">
                  <Label>Role grants</Label>
                  <div className="grid gap-2 rounded-md border p-3">
                    {ROLES.map((role) => (
                      <div key={role.name}>
                        <label className="flex items-center gap-2 text-sm">
                          <input
                            type="checkbox"
                            checked={grants.some(
                              (g) =>
                                g.role === role.name && g.station_id === null,
                            )}
                            onChange={() => toggleGrant(role.name, null)}
                            className="size-4"
                          />
                          {role.label}
                          <span className="text-xs text-muted-foreground">
                            ({role.hint})
                          </span>
                        </label>
                        {stations.map((station) => {
                          const checked = grants.some(
                            (g) =>
                              g.role === role.name &&
                              g.station_id === station.id,
                          );
                          return (
                            <label
                              key={station.id}
                              className="ml-6 flex items-center gap-2 py-0.5 text-sm"
                            >
                              <input
                                type="checkbox"
                                checked={checked}
                                onChange={() =>
                                  toggleGrant(role.name, station.id)
                                }
                                className="size-4"
                              />
                              <span className="text-muted-foreground">
                                {station.name}
                              </span>
                            </label>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
              <DialogFooter>
                <Button
                  onClick={submit}
                  disabled={saving || !username || !password}
                >
                  {saving ? "Creating…" : "Create user"}
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
        {status.state === "ok" && (
          <div className="grid gap-6">
            {status.users.map((user) => (
              <Card key={user.id}>
                <CardHeader>
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <CardTitle className="text-base">
                        {user.display_name || user.username}
                      </CardTitle>
                      <CardDescription>
                        @{user.username}
                        {user.is_super_admin && " · super admin"}
                      </CardDescription>
                    </div>
                    <div className="flex shrink-0 gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => toggleSuper(user)}
                      >
                        {user.is_super_admin ? "Demote" : "Make admin"}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => remove(user)}
                      >
                        Delete
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="flex flex-wrap gap-1.5 text-xs">
                  {user.roles.length === 0 && (
                    <span className="text-muted-foreground">
                      No role grants
                    </span>
                  )}
                  {user.roles.map((g) => (
                    <span
                      key={`${g.role}:${g.station_id ?? "*"}`}
                      className="rounded-full border px-2 py-0.5"
                    >
                      {g.role}
                      {g.station_id && (
                        <span className="text-muted-foreground">
                          {" "}
                          →{" "}
                          {stations.find((s) => s.id === g.station_id)?.name ??
                            g.station_id}
                        </span>
                      )}
                    </span>
                  ))}
                </CardContent>
              </Card>
            ))}

            <Card>
              <CardHeader>
                <CardTitle className="text-base">Audit log</CardTitle>
                <CardDescription>
                  Recent mutations, newest first.
                </CardDescription>
              </CardHeader>
              <CardContent>
                {status.audit.length === 0 ? (
                  <p className="text-sm text-muted-foreground">Nothing yet.</p>
                ) : (
                  <ul className="grid gap-1.5 text-sm">
                    {status.audit.map((entry) => (
                      <li key={entry.id} className="flex gap-2">
                        <span className="w-40 shrink-0 truncate text-muted-foreground">
                          {entry.created_at}
                        </span>
                        <span className="w-32 shrink-0 font-medium">
                          {entry.action}
                        </span>
                        <span className="min-w-0 flex-1 truncate">
                          {entry.detail || entry.target}
                        </span>
                        <span className="shrink-0 text-muted-foreground">
                          {entry.user_id ?? "system"}
                        </span>
                      </li>
                    ))}
                  </ul>
                )}
              </CardContent>
            </Card>
          </div>
        )}
      </main>
    </div>
  );
}
