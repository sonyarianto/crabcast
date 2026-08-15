"use client";

import { useCallback, useEffect, useState } from "react";
import { Check, Inbox, ListOrdered, X } from "lucide-react";
import { useTranslation } from "react-i18next";

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
  approveRequest,
  clearEngineQueue,
  getEngineQueue,
  getRequestRules,
  listRequests,
  rejectRequest,
  skipEngineQueue,
  updateRequestRules,
  type RequestEntry,
  type RequestRules,
} from "@/lib/api";

type RulesForm = Omit<RequestRules, "station_id">;

export function RequestsCard({ stationId }: { stationId: string }) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<RulesForm | null>(null);
  const [pending, setPending] = useState<RequestEntry[]>([]);
  const [recent, setRecent] = useState<RequestEntry[]>([]);
  const [queue, setQueue] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    getRequestRules(stationId)
      .then((r) =>
        setRules({
          enabled: r.enabled,
          max_per_hour: r.max_per_hour,
          dedupe: r.dedupe,
          moderation: r.moderation,
        }),
      )
      .catch(() => {});
    listRequests(stationId, true)
      .then(setPending)
      .catch(() => {});
    listRequests(stationId, false)
      .then(setRecent)
      .catch(() => {});
    getEngineQueue(stationId)
      .then(setQueue)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : t("requests.engine_unreachable")),
      );
  }, [stationId, t]);

  useEffect(() => {
    reload();
  }, [reload]);

  const saveRules = async () => {
    if (!rules) return;
    try {
      await updateRequestRules(stationId, rules);
      toast.add({
        title: t("requests.rules_saved"),
        type: "success",
        timeout: 3000,
      });
      reload();
    } catch (err) {
      toast.add({
        title: t("requests.save_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const decide = async (r: RequestEntry, action: "approve" | "reject") => {
    try {
      if (action === "approve") await approveRequest(stationId, r.id);
      else await rejectRequest(stationId, r.id);
      toast.add({
        title:
          action === "approve"
            ? t("requests.approved")
            : t("requests.rejected"),
        type: "success",
        timeout: 3000,
      });
      reload();
    } catch (err) {
      toast.add({
        title: t("requests.action_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  return (
    <Card className="mt-4">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ListOrdered className="size-4" />
          {t("requests.title")}
        </CardTitle>
        <CardDescription>{t("requests.desc")}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6">
        {error && <p className="text-sm text-destructive">{error}</p>}

        {/* Rules */}
        {rules && (
          <div className="grid gap-3">
            <div className="flex items-center justify-between">
              <Label htmlFor="rq-enabled" className="text-sm">
                {t("requests.accept")}
              </Label>
              <input
                id="rq-enabled"
                type="checkbox"
                checked={rules.enabled}
                onChange={(e) =>
                  setRules({ ...rules, enabled: e.target.checked })
                }
              />
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label htmlFor="rq-max">{t("requests.max_per_hour")}</Label>
                <input
                  id="rq-max"
                  type="number"
                  min={0}
                  value={rules.max_per_hour}
                  onChange={(e) =>
                    setRules({
                      ...rules,
                      max_per_hour: Number(e.target.value),
                    })
                  }
                  className="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
                />
              </div>
              <div className="flex flex-col justify-end gap-2 pb-1">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={rules.dedupe}
                    onChange={(e) =>
                      setRules({ ...rules, dedupe: e.target.checked })
                    }
                  />
                  {t("requests.dedupe")}
                </label>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={rules.moderation}
                    onChange={(e) =>
                      setRules({ ...rules, moderation: e.target.checked })
                    }
                  />
                  {t("requests.require_approval")}
                </label>
              </div>
            </div>
            <div>
              <Button size="sm" onClick={saveRules}>
                {t("requests.save_rules")}
              </Button>
            </div>
          </div>
        )}

        {/* Moderation inbox */}
        <div className="grid gap-2">
          <p className="flex items-center gap-1 text-sm font-medium">
            <Inbox className="size-4" />
            {t("requests.pending")}
          </p>
          {pending.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("requests.nothing_pending")}</p>
          ) : (
            <ul className="divide-y">
              {pending.map((r) => (
                <li
                  key={r.id}
                  className="flex items-center justify-between gap-3 py-2"
                >
                  <div className="min-w-0">
                    <p className="truncate font-medium">
                      {r.title}
                      {r.artist && (
                        <span className="text-muted-foreground">
                          {" "}
                          — {r.artist}
                        </span>
                      )}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {r.requested_by ?? "anonymous"} ·{" "}
                      {new Date(r.created_at).toLocaleTimeString()}
                    </p>
                  </div>
                  <div className="flex shrink-0 gap-1">
                    <Button size="sm" onClick={() => decide(r, "approve")}>
                      <Check />
                      {t("requests.approve")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="text-destructive"
                      onClick={() => decide(r, "reject")}
                    >
                      <X />
                      {t("requests.reject")}
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Engine queue */}
        <div className="grid gap-2">
          <div className="flex items-center justify-between">
            <p className="flex items-center gap-1 text-sm font-medium">
              <ListOrdered className="size-4" />
              {t("requests.engine_queue", { count: queue.length })}
            </p>
            <div className="flex gap-1">
              <Button
                size="sm"
                variant="outline"
                disabled={queue.length === 0}
                onClick={async () => {
                  try {
                    await clearEngineQueue(stationId);
                    toast.add({
                      title: t("requests.queue_cleared"),
                      type: "success",
                      timeout: 3000,
                    });
                    reload();
                  } catch (err) {
                    toast.add({
                      title: t("requests.clear_failed"),
                      description:
                        err instanceof Error ? err.message : t("common.unknown_error"),
                      type: "error",
                      timeout: 6000,
                    });
                  }
                }}
              >
                {t("requests.clear")}
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={queue.length === 0}
                onClick={async () => {
                  try {
                    await skipEngineQueue(stationId);
                    toast.add({
                      title: t("requests.queue_skipped"),
                      type: "success",
                      timeout: 3000,
                    });
                    reload();
                  } catch (err) {
                    toast.add({
                      title: t("requests.skip_failed"),
                      description:
                        err instanceof Error ? err.message : t("common.unknown_error"),
                      type: "error",
                      timeout: 6000,
                    });
                  }
                }}
              >
                {t("requests.skip_current")}
              </Button>
            </div>
          </div>
          {queue.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("requests.queue_empty")}</p>
          ) : (
            <ul className="max-h-40 divide-y overflow-auto rounded-md border">
              {queue.map((item) => (
                <li
                  key={item}
                  className="truncate px-3 py-1.5 text-sm"
                  title={item}
                >
                  {item.split("/").pop()}
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Recent history */}
        <div className="grid gap-2">
          <p className="text-sm font-medium">{t("requests.recent")}</p>
          {recent.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("requests.none")}</p>
          ) : (
            <ul className="divide-y">
              {recent.slice(0, 10).map((r) => (
                <li
                  key={r.id}
                  className="flex items-center justify-between gap-3 py-1.5 text-sm"
                >
                  <span className="truncate">{r.title}</span>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {r.status} · {new Date(r.created_at).toLocaleTimeString()}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
