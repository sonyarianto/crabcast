"use client";

import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { fetchMe, type Me } from "@/lib/api";

export type MeState =
  { state: "loading" } | { state: "ready"; me: Me } | { state: "anonymous" };

/**
 * Loads the current session once. `require` redirects unauthenticated
 * visitors to /login.
 */
export function useMe(require = true): {
  meState: MeState;
  refresh: () => Promise<void>;
} {
  const navigate = useNavigate();
  const [meState, setMeState] = useState<MeState>({ state: "loading" });

  const refresh = useCallback(async () => {
    try {
      const me = await fetchMe();
      setMeState({ state: "ready", me });
    } catch {
      setMeState({ state: "anonymous" });
      if (require) navigate("/login", { replace: true });
    }
  }, [require, navigate]);

  useEffect(() => {
    let cancelled = false;
    fetchMe()
      .then((me) => {
        if (!cancelled) setMeState({ state: "ready", me });
      })
      .catch(() => {
        if (cancelled) return;
        setMeState({ state: "anonymous" });
        if (require) navigate("/login", { replace: true });
      });
    return () => {
      cancelled = true;
    };
  }, [require, navigate]);

  return { meState, refresh };
}
