-- On-air notifications (Phase 11): per-station webhooks (Slack/Discord)
-- fired on station events. events is a comma-separated subset of
-- started,stopped,crashed,blank, or '*' for all.
CREATE TABLE notification_webhooks (
  id TEXT PRIMARY KEY,
  station_id TEXT NOT NULL REFERENCES stations(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  events TEXT NOT NULL DEFAULT '*',
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_notification_webhooks_station
  ON notification_webhooks(station_id);
