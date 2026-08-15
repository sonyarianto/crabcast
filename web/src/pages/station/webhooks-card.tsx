"use client";

import { useCallback, useEffect, useState } from "react";
import { BellRing, Plus, Trash2 } from "lucide-react";

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

const EVENT_LABELS: Record<string, string> = {
  started: "On air",
  stopped: "Off air",
  crashed: "Crashed",
  blank: "Dead air",
};

export function WebhooksCard({ stationId }: { stationId: string }) {
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
        title: "Webhook URL is required",
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
      toast.add({ title: "Webhook added", type: "success", timeout: 3000 });
    } catch (err) {
      toast.add({
        title: "Failed to add webhook",
        description: err instanceof Error ? err.message : "Unknown error",
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
        title: "Failed to delete webhook",
        description: err instanceof Error ? err.message : "Unknown error",
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
          Notifications
        </CardTitle>
        <CardDescription>
          Slack / Discord webhooks notified when the station goes on or off air,
          crashes, or has dead air. Paste an incoming-webhook URL.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        {webhooks.length === 0 && (
          <p className="text-sm text-muted-foreground">No webhooks yet.</p>
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
                  ? "All events"
                  : wh.events
                      .split(",")
                      .map((e) => EVENT_LABELS[e] ?? e)
                      .join(", ")}
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => remove(wh.id)}
              aria-label="Delete webhook"
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        ))}

        <div className="grid gap-2 border-t pt-4">
          <Label htmlFor="wh-url">Incoming webhook URL</Label>
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
                {EVENT_LABELS[event]}
              </label>
            ))}
            {events.length === 0 && (
              <span className="text-xs text-muted-foreground">
                (none selected = all events)
              </span>
            )}
          </div>
          <Button onClick={add} disabled={saving} className="mt-2 w-fit">
            <Plus />
            {saving ? "Adding…" : "Add webhook"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
