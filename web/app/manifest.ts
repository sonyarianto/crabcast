import type { MetadataRoute } from "next";

// PWA manifest: lets admins install Crabcast and run stations from their
// phone's home screen (the admin UI is responsive and touch-friendly).
export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Crabcast",
    short_name: "Crabcast",
    description:
      "Radio management platform — stations, playlists, live DJs, requests, analytics.",
    id: "/",
    start_url: "/",
    scope: "/",
    display: "standalone",
    orientation: "any",
    background_color: "#09090b",
    theme_color: "#7c3aed",
    icons: [
      { src: "/icons/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icons/icon-512.png", sizes: "512x512", type: "image/png" },
      {
        src: "/icons/maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
