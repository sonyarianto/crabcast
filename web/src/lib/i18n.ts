import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "@/locales/en.json";
import id from "@/locales/id.json";

export const STORAGE_KEY = "crabcast.language";

function initialLanguage(): string {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === "en" || saved === "id") return saved;
  const nav = (navigator.language ?? "").toLowerCase();
  return nav.startsWith("id") ? "id" : "en";
}

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    id: { translation: id },
  },
  lng: initialLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

/** Switch language, persist the choice and keep `lang`/`dir` in sync. */
export function setLanguage(lng: "en" | "id"): void {
  i18n.changeLanguage(lng);
  localStorage.setItem(STORAGE_KEY, lng);
  document.documentElement.lang = lng;
  // dir() flips to rtl for right-to-left locales when one is added later.
  document.documentElement.dir = i18n.dir(lng);
}

// Reflect the boot language on <html> before first paint.
document.documentElement.lang = i18n.language;
document.documentElement.dir = i18n.dir();

export default i18n;
