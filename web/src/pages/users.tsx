"use client";

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import { Radio, UsersIcon } from "lucide-react";
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
    labelKey: "users.role_station_manager",
    hintKey: "users.role_station_manager_hint",
  },
  {
    name: "dj",
    labelKey: "users.role_dj",
    hintKey: "users.role_dj_hint",
  },
  {
    name: "media_editor",
    labelKey: "users.role_media_editor",
    hintKey: "users.role_media_editor_hint",
  },
] as const;

export default function UsersPage() {
  const { t } = useTranslation();
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
          message: err instanceof Error ? err.message : t("common.unknown_error"),
        }),
      );
  }, [t]);

  useEffect(() => {
    Promise.all([listUsers(), listAudit()])
      .then(([users, audit]) => setStatus({ state: "ok", users, audit }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : t("common.unknown_error"),
        }),
      );
    listStations()
      .then(setStations)
      .catch(() => setStations([]));
  }, [t]);

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
      toast.add({ title: t("users.created"), type: "success", timeout: 3000 });
      setUsername("");
      setDisplayName("");
      setPassword("");
      setIsSuperAdmin(false);
      setGrants([]);
      reload();
    } catch (err) {
      toast.add({
        title: t("users.create_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const remove = async (user: UserWithRoles) => {
    if (!confirm(t("users.confirm_delete", { username: user.username }))) return;
    try {
      await deleteUser(user.id);
      toast.add({ title: t("users.deleted"), type: "success", timeout: 3000 });
      reload();
    } catch (err) {
      toast.add({
        title: t("users.delete_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
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
        title: t("users.update_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  if (meState.state === "loading") {
    return (
      <div className="flex flex-1 flex-col items-center justify-center">
        <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
      </div>
    );
  }

  if (!canAdmin) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center">
        <p className="text-sm text-destructive">{t("users.super_admin_required")}</p>
        <Link to="/stations" className="mt-2 text-sm underline">
          {t("users.back_to_stations")}
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
          <Button variant="ghost" size="sm" render={<Link to="/settings" />}>
            {t("nav.settings")}
          </Button>
          <span className="text-sm text-muted-foreground">
            {meState.state === "ready" && meState.me.user.display_name}
          </span>
          <LanguageToggle />
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto w-full max-w-4xl flex-1 px-4 py-8">
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">{t("nav.users")}</h1>
            <p className="text-sm text-muted-foreground">{t("users.subtitle")}</p>
          </div>
          <Dialog>
            <DialogTrigger render={<Button />}>
              <UsersIcon />
              {t("users.new_user")}
            </DialogTrigger>
            <DialogContent className="sm:max-w-lg">
              <DialogHeader>
                <DialogTitle>{t("users.new_user")}</DialogTitle>
                <DialogDescription>{t("users.new_user_desc")}</DialogDescription>
              </DialogHeader>
              <div className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="username">{t("login.username")}</Label>
                  <input
                    id="username"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="display_name">{t("login.display_name")}</Label>
                  <input
                    id="display_name"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="password">{t("login.password")}</Label>
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
                  {t("users.super_admin")}
                </label>
                <div className="grid gap-2">
                  <Label>{t("users.role_grants")}</Label>
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
                          {t(role.labelKey)}
                          <span className="text-xs text-muted-foreground">
                            ({t(role.hintKey)})
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
                  {saving ? t("users.creating") : t("users.create")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>

        {status.state === "loading" && (
          <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
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
                        {user.is_super_admin && ` · ${t("users.super_admin_badge")}`}
                      </CardDescription>
                    </div>
                    <div className="flex shrink-0 gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => toggleSuper(user)}
                      >
                        {user.is_super_admin ? t("users.demote") : t("users.make_admin")}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => remove(user)}
                      >
                        {t("common.delete")}
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="flex flex-wrap gap-1.5 text-xs">
                  {user.roles.length === 0 && (
                    <span className="text-muted-foreground">{t("users.no_grants")}</span>
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
                <CardTitle className="text-base">{t("users.audit_log")}</CardTitle>
                <CardDescription>{t("users.audit_desc")}</CardDescription>
              </CardHeader>
              <CardContent>
                {status.audit.length === 0 ? (
                  <p className="text-sm text-muted-foreground">{t("users.audit_empty")}</p>
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
                          {entry.user_id ?? t("users.system")}
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
