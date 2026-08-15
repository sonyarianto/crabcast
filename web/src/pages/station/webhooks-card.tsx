"use client";

import { useCallback, useEffect, useState } from "react";
import { BellRing, Plus, Trash2 } from "lucide-react";
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
  WEBHOOK_EVENTS,
  createWebhook,
  deleteWebhook,
  listWebhooks,
  type NotificationWebhook,
} from "@/lib/api";

const EVENT_KEYS: Record<string, string> = {
  started: "webhooks.event_started",
  stopped: "webhooks.event_stopped",
  crashed: "webhooks.event_crashed",
  blank: "webhooks.event_blank",
};

export function WebhooksCard({ stationId }: { stationId: string }) {
  const { t } = useTranslation();
  const [webhooks, setWebhooks] = useState<NotificationWebhook[]>([]);
  const [url, setUrl] = useState("");
  const [events, setEvents] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(() => {
    listWebhooks(stationId)
      .then(setWebhooks)
      .catch(() => {});
  }, [stationId]);

  useEffect(() => {
    reload();
  }, [reload]);

  const toggleEvent = (event: string) => {
    setEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event],
    );
  };

  const add = async () => {
    if (!url.trim()) {
      toast.add({
        title: t("webhooks.url_required"),
        type: "error",
        timeout: 4000,
      });
      return;
    }
    setSaving(true);
    try {
      await createWebhook(stationId, {
        url: url.trim(),
        events: events.length ? events.join(",") : "*",
        enabled: true,
      });
      setUrl("");
      setEvents([]);
      reload();
      toast.add({ title: t("webhooks.added"), type: "success", timeout: 3000 });
    } catch (err) {
      toast.add({
        title: t("webhooks.add_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string) => {
    try {
      await deleteWebhook(id);
      reload();
    } catch (err) {
      toast.add({
        title: t("webhooks.delete_failed"),
        description: err instanceof Error ? err.message : t("common.unknown_error"),
        type: "error",
        timeout: 6000,
      });
    }
  };

  const input =
    "h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50";

  return (
    <Card className="mt-4">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <BellRing className="size-4" />
          {t("webhooks.title")}
        </CardTitle>
        <CardDescription>{t("webhooks.desc")}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {webhooks.length === 0 && (
          <p className="text-sm text-muted-foreground">{t("webhooks.none")}</p>
        )}
        {webhooks.map((wh) => (
          <div
            key={wh.id}
            className="flex items-center justify-between gap-3 rounded-md border p-3 text-sm"
          >
            <div className="min-w-0">
              <p className="truncate font-medium">{wh.url}</p>
              <p className="text-xs text-muted-foreground">
                {wh.events === "*"
                  ? t("webhooks.all_events")
                  : wh.events
                      .split(",")
                      .map((e) => (EVENT_KEYS[e] ? t(EVENT_KEYS[e]) : e))
                      .join(", ")}
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => remove(wh.id)}
              aria-label={t("webhooks.delete_aria")}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        ))}

        <div className="grid gap-2 border-t pt-4">
          <Label htmlFor="wh-url">{t("webhooks.url_label")}</Label>
          <input
            id="wh-url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://hooks.slack.com/services/… or https://discord.com/api/webhooks/…"
            className={input}
          />
          <div className="flex flex-wrap gap-2 pt-1">
            {WEBHOOK_EVENTS.map((event) => (
              <label key={event} className="flex items-center gap-1.5 text-sm">
                <input
                  type="checkbox"
                  checked={events.includes(event)}
                  onChange={() => toggleEvent(event)}
                  className="size-4 accent-[#7c3aed]"
                />
                {t(EVENT_KEYS[event])}
              </label>
            ))}
            {events.length === 0 && (
              <span className="text-xs text-muted-foreground">
                {t("webhooks.none_selected")}
              </span>
            )}
          </div>
          <Button onClick={add} disabled={saving} className="mt-2 w-fit">
            <Plus />
            {saving ? t("webhooks.adding") : t("webhooks.add")}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
