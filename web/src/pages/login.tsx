"use client";

import { FormEvent, useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { Radio } from "lucide-react";
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
import { bootstrapAdmin, fetchMe, login } from "@/lib/api";

type Mode = "checking" | "login" | "bootstrap";

export default function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [mode, setMode] = useState<Mode>("checking");
  const [busy, setBusy] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [displayName, setDisplayName] = useState("");

  useEffect(() => {
    // Already signed in? Skip the login screen (the wizard bounces to
    // /stations when stations already exist).
    fetchMe()
      .then(() => navigate("/welcome", { replace: true }))
      .catch(async () => {
        const res = await fetch("/api/auth/setup");
        const body = res.ok ? await res.json() : null;
        setMode(body?.needed ? "bootstrap" : "login");
      });
  }, [navigate]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      if (mode === "bootstrap") {
        if (password !== confirm) throw new Error(t("login.passwords_mismatch"));
        await bootstrapAdmin({
          username,
          password,
          display_name: displayName || undefined,
        });
      } else {
        await login({ username, password });
      }
      toast.add({
        title:
          mode === "bootstrap" ? t("login.admin_created") : t("login.signed_in"),
        type: "success",
        timeout: 3000,
      });
      navigate("/welcome", { replace: true });
    } catch (err) {
      toast.add({
        title: t("login.signin_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setBusy(false);
    }
  };

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

      <main className="mx-auto flex w-full max-w-sm flex-1 flex-col justify-center px-4 py-16">
        {mode === "checking" && (
          <p className="text-sm text-muted-foreground">{t("login.checking")}</p>
        )}
        {mode !== "checking" && (
          <Card>
            <CardHeader>
              <CardTitle>
                {mode === "bootstrap"
                  ? t("login.create_first_admin")
                  : t("login.signin")}
              </CardTitle>
              <CardDescription>
                {mode === "bootstrap"
                  ? t("login.bootstrap_hint")
                  : t("login.use_credentials")}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={submit} className="grid gap-4">
                {mode === "bootstrap" && (
                  <div className="grid gap-2">
                    <Label htmlFor="display_name">{t("login.display_name")}</Label>
                    <input
                      id="display_name"
                      value={displayName}
                      onChange={(e) => setDisplayName(e.target.value)}
                      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    />
                  </div>
                )}
                <div className="grid gap-2">
                  <Label htmlFor="username">{t("login.username")}</Label>
                  <input
                    id="username"
                    autoComplete="username"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="password">{t("login.password")}</Label>
                  <input
                    id="password"
                    type="password"
                    autoComplete={
                      mode === "bootstrap" ? "new-password" : "current-password"
                    }
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                {mode === "bootstrap" && (
                  <div className="grid gap-2">
                    <Label htmlFor="confirm">{t("login.confirm_password")}</Label>
                    <input
                      id="confirm"
                      type="password"
                      autoComplete="new-password"
                      value={confirm}
                      onChange={(e) => setConfirm(e.target.value)}
                      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    />
                  </div>
                )}
                <Button type="submit" disabled={busy}>
                  {busy
                    ? t("common.please_wait")
                    : mode === "bootstrap"
                      ? t("login.create_admin")
                      : t("login.signin")}
                </Button>
              </form>
            </CardContent>
          </Card>
        )}
      </main>
    </div>
  );
}
