"use client";

import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import {
  ArchiveIcon,
  CopyIcon,
  DownloadIcon,
  KeyRoundIcon,
  PlusIcon,
  Radio,
  Trash2Icon,
  UploadIcon,
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
  backupDownloadUrl,
  createToken,
  listTokens,
  logout,
  restoreBackup,
  revokeToken,
  type ApiToken,
} from "@/lib/api";
import { useMe } from "@/lib/use-me";

export default function SettingsPage() {
  const { t } = useTranslation();
  const { meState } = useMe();
  const [tokens, setTokens] = useState<ApiToken[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [newSecret, setNewSecret] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [restoreNote, setRestoreNote] = useState<string | null>(null);

  const restore = async (file: File | undefined) => {
    if (!file) return;
    if (!window.confirm(t("settings.confirm_restore"))) return;
    setRestoring(true);
    setRestoreNote(null);
    try {
      const result = await restoreBackup(file);
      setRestoreNote(result.message);
    } catch (err) {
      toast.add({
        title: t("settings.restore_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 8000,
      });
    } finally {
      setRestoring(false);
    }
  };

  const reload = useCallback(() => {
    listTokens()
      .then(setTokens)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : t("common.unknown_error")),
      );
  }, [t]);

  useEffect(reload, [reload]);

  const create = async () => {
    setCreating(true);
    try {
      const created = await createToken(name.trim() || "default");
      setTokens((prev) => [created, ...(prev ?? [])]);
      setNewSecret(created.secret);
      setName("");
    } catch (err) {
      toast.add({
        title: t("settings.token_create_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setCreating(false);
    }
  };

  const revoke = async (id: string) => {
    try {
      await revokeToken(id);
      setTokens((prev) => prev?.filter((t) => t.id !== id) ?? null);
      toast.add({ title: t("settings.token_revoked"), type: "success", timeout: 3000 });
    } catch (err) {
      toast.add({
        title: t("settings.token_revoke_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const copySecret = async () => {
    if (!newSecret) return;
    await navigator.clipboard.writeText(newSecret);
    toast.add({ title: t("settings.copied"), type: "success", timeout: 2000 });
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

  if (error && !tokens) {
    return (
      <Shell {...shellProps}>
        <p className="text-sm text-destructive">{error}</p>
      </Shell>
    );
  }

  return (
    <Shell {...shellProps}>
      <div className="mb-6">
        <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
          <KeyRoundIcon className="size-6" />
          {t("nav.settings")}
        </h1>
        <p className="text-sm text-muted-foreground">
          {t("settings.subtitle_prefix")}{" "}
          <code className="rounded bg-muted px-1">Authorization: Bearer</code>.
        </p>
      </div>

      {newSecret && (
        <Card className="mb-4 border-primary/40">
          <CardHeader>
            <CardTitle className="text-base">{t("settings.token_created")}</CardTitle>
            <CardDescription>{t("settings.token_created_desc")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <code className="min-w-0 flex-1 truncate rounded-lg bg-muted px-3 py-2 text-sm">
                {newSecret}
              </code>
              <Button variant="outline" size="sm" onClick={copySecret}>
                <CopyIcon />
                {t("settings.copy")}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setNewSecret(null)}
              >
                {t("settings.done")}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {meState.state === "ready" && meState.me.user.is_super_admin && (
        <Card className="mb-4">
          <CardHeader>
            <CardTitle className="text-base">{t("settings.backup_title")}</CardTitle>
            <CardDescription>{t("settings.backup_desc")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                render={<a href={backupDownloadUrl()} />}
              >
                <DownloadIcon />
                {t("settings.download_backup")}
              </Button>
              <label className="inline-flex h-9 cursor-pointer items-center justify-center gap-2 rounded-md border border-input px-4 text-sm font-medium shadow-xs transition-colors outline-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring">
                <UploadIcon />
                {restoring ? t("settings.restoring") : t("settings.restore")}
                <input
                  type="file"
                  accept=".zip"
                  className="hidden"
                  disabled={restoring}
                  onChange={(e) => {
                    void restore(e.target.files?.[0]);
                    e.target.value = "";
                  }}
                />
              </label>
            </div>
            {restoreNote && (
              <p className="flex items-center gap-2 text-sm text-amber-500">
                <ArchiveIcon className="size-4" />
                {restoreNote} {t("settings.restore_note_tail")}
              </p>
            )}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t("settings.tokens_title")}</CardTitle>
          <CardDescription>{t("settings.tokens_desc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="mb-4 flex gap-2">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("settings.token_name_placeholder")}
              onKeyDown={(e) => {
                if (e.key === "Enter") void create();
              }}
              className="h-9 flex-1 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors outline-none placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Button onClick={create} disabled={creating || !name.trim()}>
              <PlusIcon />
              {t("settings.create_token")}
            </Button>
          </div>
          {tokens === null ? (
            <p className="text-sm text-muted-foreground">{t("common.loading")}</p>
          ) : tokens.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("settings.no_tokens")}</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("settings.col_name")}</TableHead>
                  <TableHead>{t("settings.col_created")}</TableHead>
                  <TableHead>{t("settings.col_last_used")}</TableHead>
                  <TableHead className="text-right">{t("settings.col_status")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tokens.map((token) => (
                  <TableRow key={token.id}>
                    <TableCell className="font-medium">{token.name}</TableCell>
                    <TableCell className="whitespace-nowrap text-muted-foreground">
                      {new Date(token.created_at).toLocaleString()}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-muted-foreground">
                      {token.last_used_at
                        ? new Date(token.last_used_at).toLocaleString()
                        : t("settings.never")}
                    </TableCell>
                    <TableCell className="text-right">
                      {token.revoked_at ? (
                        <span className="text-xs text-muted-foreground">
                          {t("settings.revoked")}
                        </span>
                      ) : (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => void revoke(token.id)}
                        >
                          <Trash2Icon />
                          {t("settings.revoke")}
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </Shell>
  );
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
  const { t } = useTranslation();
  return (
    <div className="flex flex-1 flex-col">
      <header className="flex h-14 items-center justify-between border-b px-4">
        <div className="flex items-center gap-2 font-semibold">
          <Radio className="size-5" />
          {t("app.name")}
        </div>
        <div className="flex items-center gap-3">
          {me?.isSuperAdmin && (
            <Button variant="ghost" size="sm" render={<Link to="/users" />}>
              {t("nav.users")}
            </Button>
          )}
          <Button variant="ghost" size="sm" render={<Link to="/library" />}>
            {t("nav.library")}
          </Button>
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
      <main className="mx-auto w-full max-w-3xl flex-1 px-4 py-8">
        {children}
      </main>
    </div>
  );
}
