import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router";

import { PwaRegister } from "@/components/pwa-register";
import { ThemeProvider } from "@/components/theme-provider";

import "./globals.css";

import Home from "@/pages/home";
import Library from "@/pages/library";
import Login from "@/pages/login";
import Settings from "@/pages/settings";
import Stations from "@/pages/stations";
import Users from "@/pages/users";
import Welcome from "@/pages/welcome";
import AnalyticsPage from "@/pages/station/analytics";
import StationPage from "@/pages/station/index";
import PlaylistsPage from "@/pages/station/playlists";
import PodcastsPage from "@/pages/station/podcasts";
import PublicStationPage from "@/pages/station/public";
import StationWidget from "@/pages/station/widget";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider
      attribute="class"
      defaultTheme="system"
      enableSystem
      disableTransitionOnChange
    >
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/login" element={<Login />} />
          <Route path="/welcome" element={<Welcome />} />
          <Route path="/stations" element={<Stations />} />
          <Route path="/stations/:id" element={<StationPage />} />
          <Route path="/stations/:id/analytics" element={<AnalyticsPage />} />
          <Route path="/stations/:id/playlists" element={<PlaylistsPage />} />
          <Route path="/stations/:id/podcasts" element={<PodcastsPage />} />
          <Route path="/stations/:id/public" element={<PublicStationPage />} />
          <Route path="/stations/:id/widget" element={<StationWidget />} />
          <Route path="/library" element={<Library />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/users" element={<Users />} />
        </Routes>
      </BrowserRouter>
      <PwaRegister />
    </ThemeProvider>
  </StrictMode>,
);
