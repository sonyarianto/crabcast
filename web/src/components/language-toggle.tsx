"use client";

import { useTranslation } from "react-i18next";

import { setLanguage } from "@/lib/i18n";
import { Button } from "@/components/ui/button";

export function LanguageToggle() {
  const { i18n } = useTranslation();
  const current = i18n.language.startsWith("id") ? "id" : "en";

  return (
    <div className="flex items-center rounded-md border">
      {(["en", "id"] as const).map((lng) => (
        <Button
          key={lng}
          variant="ghost"
          size="sm"
          aria-pressed={current === lng}
          className={`h-7 rounded-md px-2 text-xs ${
            current === lng ? "bg-accent text-accent-foreground" : "text-muted-foreground"
          }`}
          onClick={() => setLanguage(lng)}
        >
          {lng.toUpperCase()}
        </Button>
      ))}
    </div>
  );
}
