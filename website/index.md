---
layout: home

hero:
  name: Crabcast
  text: Web radio management for the self-hosted
  tagline: Multi-station, playlist automation, live DJs, listener requests, jingles, analytics — a Rust API plus the Crabsoup engine, with a modern Next.js admin.
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: View on GitHub
      link: https://github.com/sonyarianto/crabcast

features:
  - title: One process per station
    details: The Rust API supervises a single Crabsoup engine per station — config generated from the DB, validated with crabsoup --check, restarted with backoff on crash.
  - title: AutoDJ that just works
    details: Playlists with shuffle, loop, weights and dayparted schedules; gapless level-aware crossfades; DSP (replaygain normalization) baked into the engine.
  - title: Live DJs, ducked
    details: Per-DJ source passwords on an Icecast harbor; while a DJ is live the playlist ducks out and fades back in on disconnect — visible in real time.
  - title: Requests & jingles
    details: Rate-limited, deduplicated listener requests mapped to the engine queue, a moderation inbox, and one-shot jingles triggered from the dashboard.
  - title: Analytics & alerts
    details: Per-minute listener samples from the Icecast admin API, top songs, uptime, CSV export — and alerts for dead air, crash loops, disk and Icecast outages.
  - title: Real-time by default
    details: SSE pushes track changes; the admin dashboard, public pages and an embeddable player widget all update live without refresh buttons.
---
