export type Health = {
  status: "ok" | "error";
  version: string;
  db: "ok" | "error";
};

export type Station = {
  id: string;
  name: string;
  description: string;
  created_at: string;
  sample_rate: number;
  channels: number;
  frames_per_buffer: number;
  crossfade_seconds: number;
  fade_curve: number;
  duck_seconds: number;
  playlist_dir: string;
  jingles_dir: string;
  harbor_port: number;
  harbor_mount: string;
  harbor_password: string;
  control_port: number;
  control_http_port: number;
  icecast_host: string;
  icecast_port: number;
  icecast_mount: string;
  icecast_format: string;
  icecast_bitrate: number;
  icecast_source_user: string;
  icecast_source_password: string;
  hls_enabled: boolean;
  hls_dir: string;
  hls_segment_seconds: number;
  hls_retention: number;
  website: string;
  facebook: string;
  twitter: string;
  instagram: string;
};

export type StationInput = {
  name: string;
  description?: string;
  sample_rate?: number;
  channels?: number;
  frames_per_buffer?: number;
  crossfade_seconds?: number;
  fade_curve?: number;
  duck_seconds?: number;
  playlist_dir: string;
  jingles_dir?: string;
  harbor_port?: number;
  harbor_mount?: string;
  harbor_password?: string;
  control_port?: number;
  control_http_port?: number;
  icecast_host?: string;
  icecast_port?: number;
  icecast_mount?: string;
  icecast_format?: string;
  icecast_bitrate?: number;
  icecast_source_user?: string;
  icecast_source_password?: string;
  hls_enabled?: boolean;
  hls_dir?: string;
  hls_segment_seconds?: number;
  hls_retention?: number;
  website?: string;
  facebook?: string;
  twitter?: string;
  instagram?: string;
};

export type ProcessState = "running" | "stopped" | "failed";

export type StationStatus = {
  process: ProcessState;
  pid: number | null;
  uptime_seconds: number | null;
  restarts: number;
  last_error: string | null;
  playing: string | null;
  engine_uptime_seconds: number | null;
  engine_ok: boolean;
  /** True while a live DJ holds the harbor; the playlist is ducked. */
  live: boolean;
  history: SongHistory[];
};

export type SongHistory = {
  id: number;
  station_id: string;
  title: string;
  started_at: string;
  ended_at: string | null;
};

export type User = {
  id: string;
  username: string;
  display_name: string;
  is_super_admin: boolean;
  created_at: string;
};

export type RoleGrant = {
  role: string;
  station_id: string | null;
};

export type UserWithRoles = User & { roles: RoleGrant[] };

export type Me = {
  user: User;
  roles: RoleGrant[];
  csrf_token: string;
};

export type AuditEntry = {
  id: number;
  user_id: string | null;
  action: string;
  target: string;
  detail: string;
  created_at: string;
};

export type MediaFile = {
  id: string;
  sha256: string;
  filename: string;
  mime: string;
  size_bytes: number;
  title: string;
  artist: string;
  album: string;
  genre: string;
  duration_seconds: number | null;
  sample_rate: number | null;
  channels: number | null;
  bitrate: number | null;
  replaygain_track_gain: number | null;
  replaygain_album_gain: number | null;
  has_cover: boolean;
  waveform?: number[];
  created_at: string;
  updated_at: string;
};

export type MediaList = {
  items: MediaFile[];
  total: number;
};

export type MediaFacets = {
  artists: string[];
  albums: string[];
  genres: string[];
};

export type UploadResult = {
  filename: string;
  status: "created" | "duplicate" | "error";
  id: string | null;
  message: string | null;
};

export type MediaQuery = {
  q?: string;
  artist?: string;
  album?: string;
  genre?: string;
  sort?: string;
  order?: "asc" | "desc";
  limit?: number;
  offset?: number;
};

export async function listMedia(
  query: MediaQuery = {},
  signal?: AbortSignal,
): Promise<MediaList> {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== "") params.set(key, String(value));
  }
  const qs = params.toString();
  return request<MediaList>(
    `/api/media${qs ? `?${qs}` : ""}`,
    undefined,
    signal,
  );
}

export async function getMedia(id: string): Promise<MediaFile> {
  return request<MediaFile>(`/api/media/${id}`);
}

export async function getMediaFacets(
  signal?: AbortSignal,
): Promise<MediaFacets> {
  return request<MediaFacets>("/api/media/facets", undefined, signal);
}

export async function getMediaConfig(signal?: AbortSignal): Promise<{
  storage_dir: string;
}> {
  return request<{ storage_dir: string }>(
    "/api/media/config",
    undefined,
    signal,
  );
}

export async function uploadMedia(files: File[]): Promise<UploadResult[]> {
  const form = new FormData();
  for (const file of files) form.append("files", file);
  // No Content-Type header: fetch sets the multipart boundary itself.
  return request<UploadResult[]>("/api/media", {
    method: "POST",
    body: form,
  });
}

export async function updateMediaTags(
  id: string,
  input: { title: string; artist: string; album: string; genre: string },
): Promise<MediaFile> {
  return request<MediaFile>(`/api/media/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deleteMedia(id: string): Promise<void> {
  await request<never>(`/api/media/${id}`, { method: "DELETE" });
}

export type Playlist = {
  id: string;
  station_id: string;
  name: string;
  kind: "standard" | "looping" | "scheduled" | "once_per_hour";
  weight: number;
  shuffle: boolean;
  enabled: boolean;
  created_at: string;
  updated_at: string;
};

export type PlaylistTrack = {
  id: string;
  media_id: string;
  position: number;
  fade_in: number | null;
  fade_out: number | null;
  cue_in: number | null;
  cue_out: number | null;
};

export type PlaylistSchedule = {
  id: string;
  days: string;
  start_time: string;
  end_time: string;
};

export type PlaylistDetail = Playlist & {
  tracks: PlaylistTrack[];
  schedules: PlaylistSchedule[];
};

export type PlaylistInput = {
  name: string;
  kind: Playlist["kind"];
  weight: number;
  shuffle: boolean;
  enabled: boolean;
};

export type TrackOverrides = {
  fade_in?: number | null;
  fade_out?: number | null;
  cue_in?: number | null;
  cue_out?: number | null;
};

export async function listPlaylists(
  stationId: string,
  signal?: AbortSignal,
): Promise<PlaylistDetail[]> {
  return request<PlaylistDetail[]>(
    `/api/stations/${stationId}/playlists`,
    undefined,
    signal,
  );
}

export async function getPlaylistPreview(
  stationId: string,
  signal?: AbortSignal,
): Promise<{ lua: string }> {
  return request<{ lua: string }>(
    `/api/stations/${stationId}/playlists/preview`,
    undefined,
    signal,
  );
}

export async function createPlaylist(
  stationId: string,
  input: PlaylistInput,
): Promise<Playlist> {
  return request<Playlist>(`/api/stations/${stationId}/playlists`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function updatePlaylist(
  id: string,
  input: PlaylistInput,
): Promise<Playlist> {
  return request<Playlist>(`/api/playlists/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deletePlaylist(id: string): Promise<void> {
  await request<never>(`/api/playlists/${id}`, { method: "DELETE" });
}

export async function addPlaylistTracks(
  playlistId: string,
  mediaIds: string[],
): Promise<{ added: number }> {
  return request<{ added: number }>(`/api/playlists/${playlistId}/tracks`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ media_ids: mediaIds }),
  });
}

export async function reorderPlaylistTracks(
  playlistId: string,
  mediaIds: string[],
): Promise<void> {
  await request<never>(`/api/playlists/${playlistId}/tracks/reorder`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ media_ids: mediaIds }),
  });
}

export async function updatePlaylistTrackOverrides(
  playlistId: string,
  mediaId: string,
  overrides: TrackOverrides,
): Promise<void> {
  await request<never>(`/api/playlists/${playlistId}/tracks/${mediaId}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(overrides),
  });
}

export async function removePlaylistTrack(
  playlistId: string,
  mediaId: string,
): Promise<void> {
  await request<never>(`/api/playlists/${playlistId}/tracks/${mediaId}`, {
    method: "DELETE",
  });
}

export async function addPlaylistSchedule(
  playlistId: string,
  input: { days: string; start_time: string; end_time: string },
): Promise<PlaylistSchedule> {
  return request<PlaylistSchedule>(`/api/playlists/${playlistId}/schedules`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deletePlaylistSchedule(
  playlistId: string,
  scheduleId: string,
): Promise<void> {
  await request<never>(`/api/playlists/${playlistId}/schedules/${scheduleId}`, {
    method: "DELETE",
  });
}

export type Streamer = {
  id: string;
  station_id: string;
  name: string;
  description: string;
  source_password: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
};

export type StreamerInput = {
  name: string;
  description?: string;
  /** Empty keeps the existing password on update. */
  source_password?: string;
  enabled?: boolean;
};

export type StreamerConnectInfo = {
  streamer: Streamer;
  mount_url: string;
  harbor_port: number;
  mount: string;
  curl_mic_test: string;
};

/** Events a notification webhook can subscribe to. */
export const WEBHOOK_EVENTS = [
  "started",
  "stopped",
  "crashed",
  "blank",
] as const;

export type NotificationWebhook = {
  id: string;
  station_id: string;
  url: string;
  events: string;
  enabled: boolean;
  created_at: string;
};

export async function listWebhooks(
  stationId: string,
  signal?: AbortSignal,
): Promise<NotificationWebhook[]> {
  return request<NotificationWebhook[]>(
    `/api/stations/${stationId}/webhooks`,
    undefined,
    signal,
  );
}

export async function createWebhook(
  stationId: string,
  input: { url: string; events: string; enabled: boolean },
): Promise<NotificationWebhook> {
  return request<NotificationWebhook>(`/api/stations/${stationId}/webhooks`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deleteWebhook(id: string): Promise<void> {
  await request<never>(`/api/webhooks/${id}`, { method: "DELETE" });
}

export async function listStreamers(
  stationId: string,
  signal?: AbortSignal,
): Promise<Streamer[]> {
  return request<Streamer[]>(
    `/api/stations/${stationId}/streamers`,
    undefined,
    signal,
  );
}

export async function createStreamer(
  stationId: string,
  input: StreamerInput,
): Promise<Streamer> {
  return request<Streamer>(`/api/stations/${stationId}/streamers`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function updateStreamer(
  id: string,
  input: StreamerInput,
): Promise<Streamer> {
  return request<Streamer>(`/api/streamers/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deleteStreamer(id: string): Promise<void> {
  await request<never>(`/api/streamers/${id}`, { method: "DELETE" });
}

export async function getStreamerConnectInfo(
  id: string,
): Promise<StreamerConnectInfo> {
  return request<StreamerConnectInfo>(`/api/streamers/${id}/connect`);
}

export type RequestRules = {
  station_id: string;
  enabled: boolean;
  max_per_hour: number;
  dedupe: boolean;
  moderation: boolean;
};

export type RequestEntry = {
  id: string;
  station_id: string;
  media_id: string;
  requested_by: string | null;
  status: "pending" | "queued" | "rejected";
  created_at: string;
  updated_at: string;
  title: string;
  artist: string;
  filename: string;
};

export type Jingle = {
  filename: string;
  size_bytes: number;
};

export async function getRequestRules(
  stationId: string,
): Promise<RequestRules> {
  return request<RequestRules>(`/api/stations/${stationId}/request-rules`);
}

export async function updateRequestRules(
  stationId: string,
  input: Omit<RequestRules, "station_id">,
): Promise<RequestRules> {
  return request<RequestRules>(`/api/stations/${stationId}/request-rules`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function createRequest(
  stationId: string,
  mediaId: string,
): Promise<{ id: string; status: string; moderated: boolean }> {
  return request(`/api/stations/${stationId}/requests`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ media_id: mediaId }),
  });
}

export async function listRequests(
  stationId: string,
  pendingOnly = false,
): Promise<RequestEntry[]> {
  return request<RequestEntry[]>(
    `/api/stations/${stationId}/requests?pending=${pendingOnly}`,
  );
}

export async function approveRequest(
  stationId: string,
  requestId: string,
): Promise<void> {
  await request<never>(
    `/api/stations/${stationId}/requests/${requestId}/approve`,
    { method: "POST" },
  );
}

export async function rejectRequest(
  stationId: string,
  requestId: string,
): Promise<void> {
  await request<never>(
    `/api/stations/${stationId}/requests/${requestId}/reject`,
    { method: "POST" },
  );
}

export async function getEngineQueue(stationId: string): Promise<string[]> {
  const body = await request<{ queue: string[] }>(
    `/api/stations/${stationId}/queue`,
  );
  return body.queue;
}

export async function clearEngineQueue(stationId: string): Promise<void> {
  await request<never>(`/api/stations/${stationId}/queue`, {
    method: "POST",
  });
}

export async function skipEngineQueue(stationId: string): Promise<void> {
  await request<never>(`/api/stations/${stationId}/queue/skip`, {
    method: "POST",
  });
}

export async function listJingles(
  stationId: string,
  signal?: AbortSignal,
): Promise<Jingle[]> {
  return request<Jingle[]>(
    `/api/stations/${stationId}/jingles`,
    undefined,
    signal,
  );
}

export async function uploadJingles(
  stationId: string,
  files: File[],
): Promise<{ uploaded: string[] }> {
  const form = new FormData();
  for (const file of files) form.append("file", file);
  return request(`/api/stations/${stationId}/jingles`, {
    method: "POST",
    body: form,
  });
}

export async function deleteJingle(
  stationId: string,
  filename: string,
): Promise<void> {
  await request<never>(
    `/api/stations/${stationId}/jingles/${encodeURIComponent(filename)}`,
    { method: "DELETE" },
  );
}

export type PublicStation = {
  id: string;
  name: string;
  description: string;
  website: string;
  facebook: string;
  twitter: string;
  instagram: string;
  requests_enabled: boolean;
  stream_url: string;
  /** Playlist URL when the station has HLS enabled, else null. */
  hls_playlist_url: string | null;
  now: SongHistory | null;
  history: SongHistory[];
};

export type PublicLibraryHit = {
  id: string;
  title: string;
  artist: string;
  filename: string;
  duration_seconds: number | null;
};

export async function getPublicStation(
  stationId: string,
  signal?: AbortSignal,
): Promise<PublicStation> {
  return request<PublicStation>(
    `/api/public/stations/${stationId}`,
    undefined,
    signal,
  );
}

export async function searchPublicLibrary(
  stationId: string,
  q: string,
  signal?: AbortSignal,
): Promise<PublicLibraryHit[]> {
  const body = await request<{ results: PublicLibraryHit[] }>(
    `/api/public/stations/${stationId}/library?q=${encodeURIComponent(q)}`,
    undefined,
    signal,
  );
  return body.results;
}

let csrfToken: string | null = null;

export function setCsrfToken(token: string | null) {
  csrfToken = token;
}

async function request<T>(
  path: string,
  init?: RequestInit,
  signal?: AbortSignal,
): Promise<T> {
  const headers = new Headers(init?.headers);
  if (init?.method && init.method !== "GET" && init.method !== "HEAD") {
    if (!csrfToken) {
      try {
        const me = await fetchMe();
        csrfToken = me.csrf_token;
      } catch {
        // session missing; let the request fail with a 401 and surface it
      }
    }
    if (csrfToken) headers.set("X-CSRF-Token", csrfToken);
  }
  const res = await fetch(path, { ...init, headers, signal });
  if (!res.ok) {
    let message = `API error: ${res.status}`;
    try {
      const body = await res.json();
      if (typeof body.error === "string") message = body.error;
    } catch {
      // non-JSON error body; keep the status-based message
    }
    throw new Error(message);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

export async function fetchHealth(signal?: AbortSignal): Promise<Health> {
  return request<Health>("/api/health", undefined, signal);
}

export async function listStations(signal?: AbortSignal): Promise<Station[]> {
  return request<Station[]>("/api/stations", undefined, signal);
}

export async function getStation(id: string): Promise<Station> {
  return request<Station>(`/api/stations/${id}`);
}

export async function createStation(input: StationInput): Promise<Station> {
  return request<Station>("/api/stations", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function updateStation(
  id: string,
  input: StationInput,
): Promise<Station> {
  return request<Station>(`/api/stations/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deleteStation(id: string): Promise<void> {
  const res = await fetch(`/api/stations/${id}`, { method: "DELETE" });
  if (!res.ok) throw new Error(`API error: ${res.status}`);
}

export async function getStationStatus(
  id: string,
  signal?: AbortSignal,
): Promise<StationStatus> {
  return request<StationStatus>(
    `/api/stations/${id}/status`,
    undefined,
    signal,
  );
}

export async function sendCommand(id: string, command: string): Promise<void> {
  await request<{ ok: boolean; message: string | null }>(
    `/api/stations/${id}/cmd`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ command }),
    },
  );
}

export async function bootstrapAdmin(input: {
  username: string;
  password: string;
  display_name?: string;
}): Promise<Me> {
  const me = await request<Me>("/api/auth/bootstrap", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
  setCsrfToken(me.csrf_token);
  return me;
}

export async function login(input: {
  username: string;
  password: string;
}): Promise<Me> {
  const me = await request<Me>("/api/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
  setCsrfToken(me.csrf_token);
  return me;
}

export async function logout(): Promise<void> {
  try {
    await request<never>("/api/auth/logout", { method: "POST" });
  } finally {
    setCsrfToken(null);
  }
}

export async function fetchMe(signal?: AbortSignal): Promise<Me> {
  const me = await request<Me>("/api/auth/me", undefined, signal);
  setCsrfToken(me.csrf_token);
  return me;
}

export async function changePassword(input: {
  current_password: string;
  new_password: string;
}): Promise<void> {
  await request<never>("/api/auth/password", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export type UserInput = {
  username: string;
  password?: string;
  display_name?: string;
  is_super_admin?: boolean;
  roles?: RoleGrant[];
};

export async function listUsers(
  signal?: AbortSignal,
): Promise<UserWithRoles[]> {
  return request<UserWithRoles[]>("/api/users", undefined, signal);
}

export async function createUser(input: UserInput): Promise<UserWithRoles> {
  return request<UserWithRoles>("/api/users", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function updateUser(
  id: string,
  input: UserInput,
): Promise<UserWithRoles> {
  return request<UserWithRoles>(`/api/users/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deleteUser(id: string): Promise<void> {
  await request<never>(`/api/users/${id}`, { method: "DELETE" });
}

export async function listAudit(
  limit = 100,
  signal?: AbortSignal,
): Promise<AuditEntry[]> {
  return request<AuditEntry[]>(`/api/audit?limit=${limit}`, undefined, signal);
}

export type ListenerPoint = {
  ts: string;
  listeners: number;
  connections: number;
  samples: number;
  reachable: number;
};

export type ListenerSeries = {
  points: ListenerPoint[];
  bucket_minutes: number;
};

export type AnalyticsSummary = {
  current_listeners: number;
  last_sample_at: string | null;
  unique_listeners_24h: number;
  uptime_percent_24h: number | null;
  plays_today: number;
  requests_today: number;
};

export type TopSong = {
  title: string;
  plays: number;
  total_seconds: number;
  last_played_at: string;
};

export type RequestDay = {
  day: string;
  total: number;
  accepted: number;
  rejected: number;
  pending: number;
};

export type Alert = {
  id: string;
  station_id: string | null;
  kind: string;
  severity: "warning" | "error";
  title: string;
  detail: string;
  created_at: string;
  resolved_at: string | null;
};

export async function getListenerSeries(
  stationId: string,
  from: string,
  to: string,
  bucketMinutes: number,
  signal?: AbortSignal,
): Promise<ListenerSeries> {
  const params = new URLSearchParams({
    from,
    to,
    bucket: String(bucketMinutes),
  });
  return request<ListenerSeries>(
    `/api/stations/${stationId}/analytics/listeners?${params}`,
    undefined,
    signal,
  );
}

export async function getAnalyticsSummary(
  stationId: string,
  signal?: AbortSignal,
): Promise<AnalyticsSummary> {
  return request<AnalyticsSummary>(
    `/api/stations/${stationId}/analytics/summary`,
    undefined,
    signal,
  );
}

export async function getTopSongs(
  stationId: string,
  days: number,
  signal?: AbortSignal,
): Promise<TopSong[]> {
  return request<TopSong[]>(
    `/api/stations/${stationId}/analytics/top-songs?days=${days}`,
    undefined,
    signal,
  );
}

export async function getRequestStats(
  stationId: string,
  days: number,
  signal?: AbortSignal,
): Promise<RequestDay[]> {
  return request<RequestDay[]>(
    `/api/stations/${stationId}/analytics/requests?days=${days}`,
    undefined,
    signal,
  );
}

export async function listAlerts(
  stationId: string,
  openOnly: boolean,
  signal?: AbortSignal,
): Promise<Alert[]> {
  return request<Alert[]>(
    `/api/alerts?station_id=${stationId}&open=${openOnly}`,
    undefined,
    signal,
  );
}

export async function resolveAlert(id: string): Promise<void> {
  await request<never>(`/api/alerts/${id}/resolve`, { method: "POST" });
}

export function historyCsvUrl(stationId: string, days: number): string {
  return `/api/stations/${stationId}/analytics/history.csv?days=${days}`;
}

export type PodcastEpisode = {
  id: string;
  station_id: string;
  media_id: string;
  title: string;
  description: string;
  created_at: string;
};

export async function listPodcasts(
  stationId: string,
  signal?: AbortSignal,
): Promise<PodcastEpisode[]> {
  return request<PodcastEpisode[]>(
    `/api/stations/${stationId}/podcasts`,
    undefined,
    signal,
  );
}

export async function createPodcastEpisode(
  stationId: string,
  input: { media_id: string; title: string; description?: string },
): Promise<PodcastEpisode> {
  return request<PodcastEpisode>(`/api/stations/${stationId}/podcasts`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
}

export async function deletePodcastEpisode(id: string): Promise<void> {
  await request<never>(`/api/podcasts/${id}`, { method: "DELETE" });
}

export function podcastRssUrl(stationId: string): string {
  return `/api/public/stations/${stationId}/podcast.rss`;
}

export type ApiToken = {
  id: string;
  user_id: string;
  name: string;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
};

export type NewToken = ApiToken & {
  /** The raw secret — shown exactly once, never again retrievable. */
  secret: string;
};

export async function listTokens(signal?: AbortSignal): Promise<ApiToken[]> {
  return request<ApiToken[]>("/api/tokens", undefined, signal);
}

export async function createToken(name: string): Promise<NewToken> {
  return request<NewToken>("/api/tokens", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
}

export async function revokeToken(id: string): Promise<void> {
  await request<never>(`/api/tokens/${id}`, { method: "DELETE" });
}

export type RestoreResult = {
  status: string;
  restarting: boolean;
  message: string;
};

/** Same-origin link that streams a full backup zip (DB + media + configs). */
export function backupDownloadUrl(): string {
  return "/api/backup/download";
}

export async function restoreBackup(file: File): Promise<RestoreResult> {
  const form = new FormData();
  form.append("file", file);
  return request<RestoreResult>("/api/backup/restore", {
    method: "POST",
    body: form,
  });
}
