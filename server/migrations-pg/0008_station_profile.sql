-- Phase 7: public page branding — optional social/profile links.

ALTER TABLE stations ADD COLUMN website TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN facebook TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN twitter TEXT NOT NULL DEFAULT '';
ALTER TABLE stations ADD COLUMN instagram TEXT NOT NULL DEFAULT '';
