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
  return request<StationStatus>(`/api/stations/${id}/status`, undefined, signal);
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

export async function listUsers(signal?: AbortSignal): Promise<UserWithRoles[]> {
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