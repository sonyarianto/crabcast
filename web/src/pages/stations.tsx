"use client";

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import { PlusIcon, Radio } from "lucide-react";
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
  labelKey: string;
  type: "text" | "number" | "password";
  placeholderKey?: string;
  colSpan?: string;
}[] = [
  {
    key: "name",
    labelKey: "stations.field_name",
    type: "text",
    placeholderKey: "stations.placeholder_name",
  },
  {
    key: "description",
    labelKey: "stations.field_description",
    type: "text",
    placeholderKey: "stations.placeholder_description",
    colSpan: "sm:col-span-2",
  },
  { key: "playlist_dir", labelKey: "stations.field_playlist_dir", type: "text" },
  { key: "jingles_dir", labelKey: "stations.field_jingles_dir", type: "text" },
  { key: "harbor_port", labelKey: "stations.field_harbor_port", type: "number" },
  { key: "harbor_mount", labelKey: "stations.field_harbor_mount", type: "text" },
  { key: "harbor_password", labelKey: "stations.field_harbor_password", type: "password" },
  { key: "control_port", labelKey: "stations.field_control_port", type: "number" },
  { key: "control_http_port", labelKey: "stations.field_control_http_port", type: "number" },
  { key: "icecast_host", labelKey: "stations.field_icecast_host", type: "text" },
  { key: "icecast_port", labelKey: "stations.field_icecast_port", type: "number" },
  { key: "icecast_mount", labelKey: "stations.field_icecast_mount", type: "text" },
  { key: "icecast_format", labelKey: "stations.field_icecast_format", type: "text" },
  { key: "icecast_bitrate", labelKey: "stations.field_icecast_bitrate", type: "number" },
  { key: "icecast_source_user", labelKey: "stations.field_icecast_source_user", type: "text" },
  {
    key: "icecast_source_password",
    labelKey: "stations.field_icecast_source_password",
    type: "password",
  },
];

export default function StationsPage() {
  const { t } = useTranslation();
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
          message: err instanceof Error ? err.message : t("common.unknown_error"),
        }),
      );
  }, [t]);

  useEffect(() => {
    listStations()
      .then((stations) => setStatus({ state: "ok", stations }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : t("common.unknown_error"),
        }),
      );
  }, [t]);

  const submit = async () => {
    setSaving(true);
    try {
      await createStation(form);
      toast.add({
        title: t("welcome.station_created"),
        type: "success",
        timeout: 3000,
      });
      setForm(defaultInput);
      reload();
    } catch (err) {
      toast.add({
        title: t("stations.failed_create"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const remove = async (station: Station) => {
    if (!confirm(t("stations.confirm_delete", { name: station.name }))) {
      return;
    }
    try {
      await deleteStation(station.id);
      toast.add({
        title: t("stations.station_deleted"),
        type: "success",
        timeout: 3000,
      });
      reload();
    } catch (err) {
      toast.add({
        title: t("stations.failed_delete"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
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
            <Button variant="ghost" size="sm" render={<Link to="/users" />}>
              {t("nav.users")}
            </Button>
          )}
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
        <div className="mb-6 flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">
              {t("stations.title")}
            </h1>
            <p className="text-sm text-muted-foreground">{t("stations.subtitle")}</p>
          </div>
          <Dialog>
            <DialogTrigger render={<Button />}>
              <PlusIcon />
              {t("stations.new_station")}
            </DialogTrigger>
            <DialogContent className="sm:max-w-lg">
              <DialogHeader>
                <DialogTitle>{t("stations.new_station")}</DialogTitle>
                <DialogDescription>{t("stations.dialog_desc")}</DialogDescription>
              </DialogHeader>
              <div className="grid gap-4 sm:grid-cols-2">
                {FIELDS.map((field) => (
                  <div
                    key={field.key}
                    className={field.colSpan ?? "grid gap-2"}
                  >
                    <Label htmlFor={field.key}>{t(field.labelKey)}</Label>
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
                      placeholder={field.placeholderKey ? t(field.placeholderKey) : undefined}
                      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    />
                  </div>
                ))}
              </div>
              <DialogFooter>
                <Button onClick={submit} disabled={saving}>
                  {saving ? t("stations.starting_engine") : t("welcome.create_station")}
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
        {status.state === "ok" && status.stations.length === 0 && (
          <Card>
            <CardHeader>
              <CardTitle>{t("stations.no_stations")}</CardTitle>
              <CardDescription>{t("stations.no_stations_desc")}</CardDescription>
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
                    <Button
                      variant="outline"
                      size="sm"
                      render={<Link to={`/stations/${station.id}`} />}
                    >
                      {t("stations.details")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => remove(station)}
                    >
                      {t("common.delete")}
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
