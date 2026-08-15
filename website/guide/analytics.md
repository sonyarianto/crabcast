# Analytics & alerts

## Listener tracking

A background poller queries each station's **Icecast admin API**
(`/admin/stats`) every minute and stores per-station samples:
current listeners, Icecast's cumulative connection counter (unique
listeners over a window ≈ its delta), and whether the admin API responded.

The `/stations/:id/analytics` page shows:

- **Listeners chart** over 24h / 7d / 30d (per-minute samples, bucketed).
- **Stat cards** — listeners now, unique (24h), uptime % (24h), plays
  today, requests today.
- **Top songs** — play counts and total air time for the window.
- **Request rates** — accepted / rejected / pending per day.
- **CSV export** of song history (`history.csv`).

::: warning Icecast admin credentials
The poller authenticates with the station's *source* credentials. Stock
Icecast requires the admin user for `/admin/`, so give the source user
admin rights — otherwise the mount stays unreachable and an
`icecast_unreachable` alert fires.
:::

## Alerts

Alerts are deduplicated per station+kind while open, resolved when the
condition clears, and optionally posted to `CRABCAST_ALERT_WEBHOOK_URL`.

| Kind | Trigger | Auto-resolves when |
| --- | --- | --- |
| `dead_air` | The generated script wraps the output in `blank.detect` (5 s of silence) and webhooks the backend; the mount stays up, just silent | A real track is heard (track webhook) |
| `engine_crash_loop` | 5+ consecutive crashes within 60 s of engine start | The engine stays up for 60 s |
| `icecast_unreachable` | The admin API stops responding | The admin API answers again |
| `disk_low` | Media storage free space < 1 GiB **or** < 5 % | Space recovers |

Resolve alerts manually from the analytics page (station managers) or
automatically via the conditions above.

## Retention

`CRABCAST_RETENTION_DAYS` (default 30) controls how long listener samples,
song history and resolved alerts are kept; the poller purges older rows
every 6 hours. Open alerts are never purged.
