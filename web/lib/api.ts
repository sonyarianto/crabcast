export type Health = {
  status: "ok" | "error";
  version: string;
  db: "ok" | "error";
};

export async function fetchHealth(signal?: AbortSignal): Promise<Health> {
  const res = await fetch("/api/health", { signal });
  if (!res.ok) throw new Error(`API health check failed: ${res.status}`);
  return res.json();
}
