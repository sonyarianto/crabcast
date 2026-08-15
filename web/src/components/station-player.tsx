import { useEffect, useRef } from "react";

/**
 * Station audio player. When the station publishes an HLS playlist it is
 * preferred (low-latency-ish, works in every modern browser via hls.js —
 * Safari/iOS use the native HLS path); otherwise it falls back to the raw
 * Icecast mount like the previous plain <audio> element. hls.js is loaded
 * lazily so non-HLS pages never pay for it.
 */
export function StationPlayer({
  streamUrl,
  hlsPlaylistUrl,
}: {
  streamUrl: string;
  hlsPlaylistUrl: string | null;
}) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const hlsRef = useRef<{ destroy: () => void } | null>(null);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    if (hlsPlaylistUrl) {
      const canNative =
        typeof audio.canPlayType === "function" &&
        audio.canPlayType("application/vnd.apple.mpegurl") !== "";
      if (canNative) {
        // Safari / iOS: native HLS support, no hls.js needed.
        audio.src = hlsPlaylistUrl;
        return;
      }
      // Chromium/Firefox: drive the <audio> element with hls.js. The
      // attach flow is async, so stop the stale element first.
      audio.pause();
      let disposed = false;
      import("hls.js")
        .then(({ default: Hls }) => {
          if (disposed || !Hls.isSupported()) {
            audio.src = hlsPlaylistUrl;
            return;
          }
          const hls = new Hls({
            enableWorker: true,
            // Phase 11 LL-HLS: with 2s segments the player syncs ~2 segments
            // behind the live edge instead of buffering 15+s of 5s chunks.
            lowLatencyMode: true,
            liveSyncDurationCount: 2,
            liveMaxLatencyDurationCount: 4,
            backBufferLength: 30,
          });
          hlsRef.current = hls;
          hls.loadSource(hlsPlaylistUrl);
          hls.attachMedia(audio);
        })
        .catch(() => {
          audio.src = hlsPlaylistUrl;
        });
      return () => {
        disposed = true;
        hlsRef.current?.destroy();
        hlsRef.current = null;
      };
    }

    audio.src = streamUrl;
    return undefined;
  }, [streamUrl, hlsPlaylistUrl]);

  return <audio ref={audioRef} controls className="w-full" />;
}
