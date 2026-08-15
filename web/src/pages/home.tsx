"use client";

import { useEffect, useState } from "react";
import { Link } from "react-router";
import { Radio } from "lucide-react";
import { useTranslation } from "react-i18next";

import { LanguageToggle } from "@/components/language-toggle";
import { ThemeToggle } from "@/components/theme-toggle";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { fetchHealth, fetchMe, listStations, type Health } from "@/lib/api";

type Status =
  | { state: "loading" }
  | { state: "ok"; health: Health }
  | { state: "error"; message: string };

export default function Home() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<Status>({ state: "loading" });
  const [freshInstall, setFreshInstall] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    fetchHealth(controller.signal)
      .then((health) => setStatus({ state: "ok", health }))
      .catch((err: unknown) =>
        setStatus({
          state: "error",
          message: err instanceof Error ? err.message : t("common.unknown_error"),
        }),
      );
    // Fresh install (signed in, no stations yet): surface the wizard.
    fetchMe(controller.signal)
      .then(() =>
        listStations(controller.signal).then((stations) =>
          setFreshInstall(stations.length === 0),
        ),
      )
      .catch(() => {
        // anonymous; nothing to offer
      });
    return () => controller.abort();
  }, [t]);

  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          {t("app.name")}
        </div>
        <div className="flex items-center gap-3">
          <LanguageToggle />
          <ThemeToggle />
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col justify-center gap-8 px-4 py-16">
        <div className="space-y-2">
          <h1 className="text-3xl font-semibold tracking-tight">
            {t("home.title")}
          </h1>
          <p className="text-muted-foreground">{t("home.subtitle")}</p>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>{t("home.api_health")}</CardTitle>
            <CardDescription>{t("home.api_health_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="flex items-center gap-3">
            <span
              className={`size-2.5 rounded-full ${
                status.state === "ok"
                  ? "bg-emerald-500"
                  : status.state === "loading"
                    ? "animate-pulse bg-amber-500"
                    : "bg-destructive"
              }`}
            />
            {status.state === "loading" && (
              <span className="text-sm text-muted-foreground">
                {t("home.checking_api")}
              </span>
            )}
            {status.state === "ok" && (
              <span className="text-sm">
                <span className="font-medium">{status.health.status}</span>
                <span className="text-muted-foreground">
                  {" · "}v{status.health.version}
                  {" · "}db {status.health.db}
                </span>
              </span>
            )}
            {status.state === "error" && (
              <span className="text-sm text-destructive">
                {t("home.api_unreachable", { message: status.message })}
              </span>
            )}
          </CardContent>
        </Card>

        <p className="text-sm text-muted-foreground">{t("home.phase_note")}</p>

        {freshInstall && (
          <Card className="border-primary/40">
            <CardHeader>
              <CardTitle>{t("home.lets_go_live")}</CardTitle>
              <CardDescription>{t("home.fresh_install_desc")}</CardDescription>
            </CardHeader>
            <CardContent>
              <Link
                to="/welcome"
                className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs transition-[color,box-shadow] outline-none hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              >
                {t("home.setup_first_station")}
              </Link>
            </CardContent>
          </Card>
        )}

        <Link
          to="/stations"
          className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground shadow-xs transition-[color,box-shadow] outline-none hover:bg-primary/90 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          {t("home.go_stations")}
        </Link>
      </main>
    </div>
  );
}
