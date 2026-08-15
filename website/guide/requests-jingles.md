# Requests & jingles

## Listener requests

Each station has per-station **request rules**:

| Rule | Effect |
| --- | --- |
| `enabled` | Whether the public request form accepts anything |
| `max_per_hour` | Rolling-hour cap (429 when exceeded) |
| `dedupe` | Reject a track already pending/queued or in the engine queue |
| `moderation` | Hold new requests in a **pending inbox** until a station manager approves |

Flow:

1. A listener searches the public library and requests a track — no account
   needed (rules still rate-limit).
2. The backend enforces the rules, then maps the request to the engine's
   `queue.push` with the track's absolute library path — it plays within
   seconds, preempting the playlist (`request.queue()` sits ahead of the
   playlist in the fallback chain).
3. Requests land in `requests` history with status `pending` / `queued` /
   `rejected`; approved ones push to the engine, rejected ones are dropped
   (and don't count toward the hourly cap).

The dashboard also exposes the **engine queue directly**: view the current
queue, clear it, or skip the playing track — handy for a remote control
surface.

## Jingles

Short clips that play over the music, one-shot:

- Upload from the station's **Jingles card** (multipart); the engine
  re-scans the jingles directory immediately, so new clips are playable by
  name.
- Preview inline, then fire **`jingles.play <name>`** on air from the
  dashboard (or any API client).
- Deleting removes the file and re-applies the config.

The generated script wires jingles as the first child of the fallback
(`fallback({j, live, rq, pl})`), so a jingle preempts the playlist and
returns to it when done.
