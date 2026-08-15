"use client";

import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { Link } from "react-router";
import {
  CheckIcon,
  Music2Icon,
  PlayIcon,
  Radio,
  RocketIcon,
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
import { Label } from "@/components/ui/label";
import { toast } from "@/components/ui/toast";
import {
  addPlaylistTracks,
  createPlaylist,
  createStation,
  getMediaConfig,
  listStations,
  logout,
  uploadMedia,
  type Station,
  type UploadResult,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

type Step = "station" | "media" | "playlist" | "done";

export default function WelcomePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { meState } = useMe();
  const [step, setStep] = useState<Step>("station");
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [station, setStation] = useState<Station | null>(null);
  const [uploaded, setUploaded] = useState<UploadResult[]>([]);
  const [playlistDir, setPlaylistDir] = useState("");

  // Fresh installs have no stations; once one exists, skip the wizard.
  useEffect(() => {
    if (meState.state !== "ready") return;
    listStations()
      .then((stations) => {
        if (stations.length > 0) navigate("/stations", { replace: true });
      })
      .catch(() => {
        // API hiccup; let the wizard render anyway.
      });
  }, [meState.state, navigate]);

  useEffect(() => {
    getMediaConfig()
      .then((cfg) => setPlaylistDir(cfg.storage_dir))
      .catch(() => {
        // Defaults apply server-side; the field is informational.
      });
  }, []);

  const createStationStep = async () => {
    if (!name.trim()) {
      toast.add({
        title: t("welcome.station_name_required"),
        type: "error",
        timeout: 4000,
      });
      return;
    }
    setBusy(true);
    try {
      const created = await createStation({
        name: name.trim(),
        description,
        playlist_dir: playlistDir,
      });
      setStation(created);
      setStep("media");
      toast.add({ title: t("welcome.station_created"), type: "success", timeout: 3000 });
    } catch (err) {
      toast.add({
        title: t("welcome.could_not_create_station"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setBusy(false);
    }
  };

  const uploadStep = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setBusy(true);
    try {
      const results = await uploadMedia(Array.from(files));
      setUploaded((prev) => [...prev, ...results]);
      const ok = results.filter((r) => r.status !== "error").length;
      toast.add({
        title: t("welcome.files_added", { count: ok }),
        type: ok > 0 ? "success" : "error",
        timeout: 4000,
      });
    } catch (err) {
      toast.add({
        title: t("welcome.upload_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setBusy(false);
    }
  };

  const finishStep = async () => {
    if (!station) return;
    const mediaIds = uploaded
      .filter((r) => r.status !== "error" && r.id)
      .map((r) => r.id as string);
    setBusy(true);
    try {
      const playlist = await createPlaylist(station.id, {
        name: "Default",
        kind: "standard",
        weight: 100,
        shuffle: true,
        enabled: true,
      });
      if (mediaIds.length > 0) {
        await addPlaylistTracks(playlist.id, mediaIds);
      }
      setStep("done");
    } catch (err) {
      toast.add({
        title: t("welcome.could_not_build_playlist"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setBusy(false);
    }
  };

  const steps: { key: Step; label: string }[] = [
    { key: "station", label: t("welcome.step_station") },
    { key: "media", label: t("welcome.step_media") },
    { key: "playlist", label: t("welcome.step_go_live") },
  ];
  const stepIndex = steps.findIndex((s) => s.key === step);

  const shellProps = {
    me:
      meState.state === "ready"
        ? {
            displayName:
              meState.me.user.display_name || meState.me.user.username,
          }
        : null,
    onLogout: async () => {
      await logout();
      window.location.reload();
    },
  } as const;

  return (
    <Shell {...shellProps}>
      <div className="mb-6">
        <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
          <RocketIcon className="size-6" />
          {t("welcome.title")}
        </h1>
        <p className="text-sm text-muted-foreground">{t("welcome.subtitle")}</p>
      </div>

      <ol className="mb-6 flex items-center gap-2 text-sm">
        {steps.map((s, i) => (
          <li key={s.key} className="flex items-center gap-2">
            <span
              className={`flex size-5 items-center justify-center rounded-full text-xs ${
                i < stepIndex || step === "done"
                  ? "bg-primary text-primary-foreground"
                  : i === stepIndex
                    ? "border border-primary text-primary"
                    : "border border-border text-muted-foreground"
              }`}
            >
              {i < stepIndex || step === "done" ? (
                <CheckIcon className="size-3" />
              ) : (
                i + 1
              )}
            </span>
            <span
              className={
                i === stepIndex ? "font-medium" : "text-muted-foreground"
              }
            >
              {s.label}
            </span>
            {i < steps.length - 1 && (
              <span className="text-muted-foreground">·</span>
            )}
          </li>
        ))}
      </ol>

      {step === "station" && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("welcome.step1_title")}</CardTitle>
            <CardDescription>{t("welcome.step1_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="name">{t("welcome.station_name")}</Label>
              <input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t("welcome.station_name_placeholder")}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="desc">{t("welcome.description_optional")}</Label>
              <input
                id="desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t("welcome.description_placeholder")}
                className="h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
              />
            </div>
            <Button onClick={createStationStep} disabled={busy}>
              {t("welcome.create_station")}
            </Button>
          </CardContent>
        </Card>
      )}

      {step === "media" && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("welcome.step2_title")}</CardTitle>
            <CardDescription>{t("welcome.step2_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <label className="inline-flex h-9 cursor-pointer items-center justify-center gap-2 rounded-md border border-input px-4 text-sm font-medium shadow-xs transition-colors outline-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring">
              <Music2Icon />
              {busy ? t("common.uploading") : t("welcome.choose_audio")}
              <input
                type="file"
                multiple
                accept="audio/*"
                className="hidden"
                disabled={busy}
                onChange={(e) => {
                  void uploadStep(e.target.files);
                  e.target.value = "";
                }}
              />
            </label>
            {uploaded.length > 0 && (
              <ul className="space-y-1 text-sm">
                {uploaded.map((r, i) => (
                  <li
                    key={`${r.filename}-${i}`}
                    className="flex items-center gap-2 text-muted-foreground"
                  >
                    <CheckIcon className="size-3.5 text-emerald-500" />
                    <span className="truncate">{r.filename}</span>
                  </li>
                ))}
              </ul>
            )}
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setStep("station")}>
                {t("common.back")}
              </Button>
              <Button
                onClick={finishStep}
                disabled={busy || uploaded.length === 0}
              >
                {t("common.continue")}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {step === "playlist" && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("welcome.step3_title")}</CardTitle>
            <CardDescription>{t("welcome.step3_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {busy && (
              <p className="animate-pulse text-sm text-muted-foreground">
                {t("welcome.step3_starting")}
              </p>
            )}
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setStep("media")}>
                {t("common.back")}
              </Button>
              <Button onClick={finishStep} disabled={busy}>
                {t("welcome.go_live")}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {step === "done" && station && (
        <Card className="border-emerald-500/40">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <PlayIcon className="size-5 text-emerald-500" />
              {t("welcome.live", { name: station.name })}
            </CardTitle>
            <CardDescription>{t("welcome.live_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            <Button render={<Link to={`/stations/${station.id}`} />}>
              {t("welcome.open_dashboard")}
            </Button>
            <Button
              variant="outline"
              render={
                <Link to={`/stations/${station.id}/public`} target="_blank" />
              }
            >
              {t("welcome.view_public")}
            </Button>
          </CardContent>
        </Card>
      )}
    </Shell>
  );
}

function Shell({
  children,
  me,
  onLogout,
}: {
  children: React.ReactNode;
  me?: { displayName: string } | null;
  onLogout?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          Crabcast
        </div>
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="sm" render={<Link to="/stations" />}>
            {t("nav.stations")}
          </Button>
          {me && (
            <>
              <span className="text-sm text-muted-foreground">
                {me.displayName}
              </span>
              <Button variant="ghost" size="sm" onClick={onLogout}>
                {t("nav.logout")}
              </Button>
            </>
          )}
          <LanguageToggle />
          <ThemeToggle />
        </div>
      </header>
      <main className="mx-auto w-full max-w-2xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  );
}
