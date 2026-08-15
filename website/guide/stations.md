# Stations, playlists & live DJs

## Stations

Each station is a row in the DB plus one supervised engine process. The
station model covers:

- **Audio settings** — sample rate, channels, frames per buffer, and the
  crossfade / fade-curve / duck durations that map to the engine's `set()`
  knobs.
- **Sources** — `playlist_dir` (AutoDJ), `jingles_dir` (one-shots), and a
  `harbor` mount (`/live`) that live DJs connect to.
- **Control ports** — a telnet port and an HTTP control port
  (`/status`, `/queue`, `/jingles`, `/cmd`).
- **Icecast target** — host, port, mount (`/radio`), format (MP3 for max
  compatibility; Opus/AAC as progressive enhancement), bitrate and source
  credentials.
- **Branding** — name, description, and website/social links surfaced on
  the public page.

Creating or editing a station re-renders its `crabsoup.lua`, runs
`crabsoup --check`, and restarts the engine **atomically** (kill old →
spawn new; the supervisor waits for the old process to actually exit, so
ports never race).

## Playlists & scheduling (AutoDJ)

Four playlist kinds cover AzuraCast-style automation:

| Kind | Behavior |
| --- | --- |
| `standard` | Shuffle or sequential rotation |
| `looping` | Sequential, always the same order |
| `scheduled` | Dayparted: `switch` slots from weekday + HH:MM rules |
| `once_per_hour` | Rotates with the always-on set |

- **Weights** bias the `rotate` between concurrent playlists.
- **Per-track overrides** — fade in/out and cue in/out — are baked into the
  generated script as `annotate:` prefixes.
- **Dayparting** composes a `switch` whose fallback child is the rest of
  the playlists, so a scheduled-only station still has an on-air default.
- Every mutation re-renders the script, `--check`s it, and restarts the
  engine — changes apply **live without dropping audio**.
- `/api/stations/:id/playlists/preview` returns the generated Lua for
  inspection.

## Streamers (live DJs)

Each station can have per-DJ accounts, each with its **own source
password**:

- A DJ connects to the harbor mount (`/live`) with their credentials from
  any Icecast source client (or the copy-paste `curl` mic test in the UI).
- While a DJ holds the harbor the playlist **ducks out**; on disconnect it
  fades back in.
- `GET /status` reports `harbor_connected` / `live`, so the dashboard shows
  a pulsing **LIVE** badge in real time.
- Disabling a streamer instantly re-renders the config — the old password
  stops authenticating on the next engine restart.

::: tip
The station page links to the **public page** (brandable player, now
playing, history, request form) and an **embeddable iframe widget** for
third-party sites.
:::
