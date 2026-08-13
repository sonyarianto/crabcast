"use client";

import { FormEvent, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Radio } from "lucide-react";

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
  const router = useRouter();
  const [mode, setMode] = useState<Mode>("checking");
  const [busy, setBusy] = useState(false);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [displayName, setDisplayName] = useState("");

  useEffect(() => {
    // Already signed in? Skip the login screen.
    fetchMe()
      .then(() => router.replace("/stations"))
      .catch(async () => {
        const res = await fetch("/api/auth/setup");
        const body = res.ok ? await res.json() : null;
        setMode(body?.needed ? "bootstrap" : "login");
      });
  }, [router]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    try {
      if (mode === "bootstrap") {
        if (password !== confirm) throw new Error("Passwords do not match");
        await bootstrapAdmin({
          username,
          password,
          display_name: displayName || undefined,
        });
      } else {
        await login({ username, password });
      }
      toast.add({
        title: mode === "bootstrap" ? "Admin created" : "Signed in",
        type: "success",
        timeout: 3000,
      });
      router.replace("/stations");
    } catch (err) {
      toast.add({
        title: "Sign-in failed",
        description: err instanceof Error ? err.message : "Unknown error",
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
          Crabcast
        </div>
        <ThemeToggle />
      </header>

      <main className="mx-auto flex w-full max-w-sm flex-1 flex-col justify-center px-4 py-16">
        {mode === "checking" && (
          <p className="text-sm text-muted-foreground">Checking…</p>
        )}
        {mode !== "checking" && (
          <Card>
            <CardHeader>
              <CardTitle>
                {mode === "bootstrap" ? "Create the first admin" : "Sign in"}
              </CardTitle>
              <CardDescription>
                {mode === "bootstrap"
                  ? "No users exist yet. This account gets super-admin rights."
                  : "Use your Crabcast username and password."}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form onSubmit={submit} className="grid gap-4">
                {mode === "bootstrap" && (
                  <div className="grid gap-2">
                    <Label htmlFor="display_name">Display name</Label>
                    <input
                      id="display_name"
                      value={displayName}
                      onChange={(e) => setDisplayName(e.target.value)}
                      className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                    />
                  </div>
                )}
                <div className="grid gap-2">
                  <Label htmlFor="username">Username</Label>
                  <input
                    id="username"
                    autoComplete="username"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                  />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="password">Password</Label>
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
                    <Label htmlFor="confirm">Confirm password</Label>
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
                    ? "Please wait…"
                    : mode === "bootstrap"
                      ? "Create admin"
                      : "Sign in"}
                </Button>
              </form>
            </CardContent>
          </Card>
        )}
      </main>
    </div>
  );
}