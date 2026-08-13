"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";

import { fetchMe, type Me } from "@/lib/api";

export type MeState =
  | { state: "loading" }
  | { state: "ready"; me: Me }
  | { state: "anonymous" };

/**
 * Loads the current session once. `require` redirects unauthenticated
 * visitors to /login.
 */
export function useMe(require = true): {
  meState: MeState;
  refresh: () => Promise<void>;
} {
  const router = useRouter();
  const [meState, setMeState] = useState<MeState>({ state: "loading" });

  const refresh = useCallback(async () => {
    try {
      const me = await fetchMe();
      setMeState({ state: "ready", me });
    } catch {
      setMeState({ state: "anonymous" });
      if (require) router.replace("/login");
    }
  }, [require, router]);

  useEffect(() => {
    let cancelled = false;
    fetchMe()
      .then((me) => {
        if (!cancelled) setMeState({ state: "ready", me });
      })
      .catch(() => {
        if (cancelled) return;
        setMeState({ state: "anonymous" });
        if (require) router.replace("/login");
      });
    return () => {
      cancelled = true;
    };
  }, [require, router]);

  return { meState, refresh };
}