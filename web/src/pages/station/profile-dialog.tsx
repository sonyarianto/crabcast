"use client";

import { useState } from "react";
import { Pencil } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { toast } from "@/components/ui/toast";
import { updateStation, type Station } from "@/lib/api";

type Profile = {
  name: string;
  description: string;
  website: string;
  facebook: string;
  twitter: string;
  instagram: string;
  hls_enabled: boolean;
  hls_dir: string;
};

export function ProfileDialog({
  station,
  onSaved,
}: {
  station: Station;
  onSaved: (updated: Station) => void;
}) {
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState<Profile>({
    name: station.name,
    description: station.description,
    website: station.website,
    facebook: station.facebook,
    twitter: station.twitter,
    instagram: station.instagram,
    hls_enabled: station.hls_enabled,
    hls_dir: station.hls_dir,
  });
  const [saving, setSaving] = useState(false);

  const openDialog = () => {
    setForm({
      name: station.name,
      description: station.description,
      website: station.website,
      facebook: station.facebook,
      twitter: station.twitter,
      instagram: station.instagram,
      hls_enabled: station.hls_enabled,
      hls_dir: station.hls_dir,
    });
    setOpen(true);
  };

  const save = async () => {
    if (!form.name.trim()) {
      toast.add({ title: "Name is required", type: "error", timeout: 4000 });
      return;
    }
    setSaving(true);
    try {
      const updated = await updateStation(station.id, {
        ...station,
        name: form.name.trim(),
        description: form.description,
        website: form.website,
        facebook: form.facebook,
        twitter: form.twitter,
        instagram: form.instagram,
        hls_enabled: form.hls_enabled,
        hls_dir: form.hls_dir.trim(),
      });
      onSaved(updated);
      setOpen(false);
      toast.add({
        title: "Station profile saved",
        type: "success",
        timeout: 3000,
      });
    } catch (err) {
      toast.add({
        title: "Save failed",
        description: err instanceof Error ? err.message : "Unknown error",
        type: "error",
        timeout: 6000,
      });
    } finally {
      setSaving(false);
    }
  };

  const input =
    "h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50";

  return (
    <>
      <Button variant="outline" size="sm" onClick={openDialog}>
        <Pencil />
        Edit profile
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Station profile</DialogTitle>
            <DialogDescription>
              Name, description and public-page social links.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="pf-name">Name</Label>
              <input
                id="pf-name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className={input}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="pf-desc">Description</Label>
              <textarea
                id="pf-desc"
                value={form.description}
                onChange={(e) =>
                  setForm({ ...form, description: e.target.value })
                }
                rows={2}
                className="w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
              />
            </div>
            {(
              [
                ["website", "Website"],
                ["facebook", "Facebook"],
                ["twitter", "X / Twitter"],
                ["instagram", "Instagram"],
              ] as const
            ).map(([key, label]) => (
              <div className="grid gap-2" key={key}>
                <Label htmlFor={`pf-${key}`}>{label}</Label>
                <input
                  id={`pf-${key}`}
                  value={form[key]}
                  onChange={(e) => setForm({ ...form, [key]: e.target.value })}
                  placeholder="https://…"
                  className={input}
                />
              </div>
            ))}
            <div className="grid gap-2">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={form.hls_enabled}
                  onChange={(e) =>
                    setForm({ ...form, hls_enabled: e.target.checked })
                  }
                  className="size-4 accent-[#7c3aed]"
                />
                HLS streaming
              </label>
              {form.hls_enabled && (
                <div className="grid gap-2">
                  <Label htmlFor="pf-hls-dir">HLS directory</Label>
                  <input
                    id="pf-hls-dir"
                    value={form.hls_dir}
                    onChange={(e) =>
                      setForm({ ...form, hls_dir: e.target.value })
                    }
                    placeholder="/srv/hls/my-station"
                    className={input}
                  />
                  <p className="text-xs text-muted-foreground">
                    Writable server path for MPEG-TS segments (created
                    automatically). The public page and widget then play the HLS
                    stream instead of the raw mount.
                  </p>
                </div>
              )}
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={save} disabled={saving}>
              {saving ? "Saving…" : "Save"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
