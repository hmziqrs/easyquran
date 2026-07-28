/* ════════════════════════════════════════════════════════════════════════
   site.ts — the single source of truth for the EasyQuran site.

   The route tree is split into two groups (see src/routes):
     • (marketing) — public, indexable pages. These drive the nav, the
       sitemap, llms.txt and the .md/.txt text variants.
     • (application) — the /app product UI. Deliberately kept OUT of the
       sitemap, llms.txt and text variants, and marked noindex via <Seo>.

   ⚠ Placeholders: `domain` / `url` / the social handles below are stand-ins.
   Set them once here and <Seo>, sitemap.xml, llms.txt and robots.txt follow.
   ════════════════════════════════════════════════════════════════════════ */

export const SITE = {
  name: "EasyQuran",
  domain: "easyquran.app",
  url: "https://easyquran.app",
  github: "https://github.com/hmziqrs",
  tagline: "the Quran, made easy to read",
} as const;

export type ThemeMode = "dark" | "light";
export type AccentId = "emerald" | "gold" | "azure" | "plum";

export type MarketingPageId = "home" | "about" | "download" | "privacy";
export type AppPageId = "app" | "read" | "bookmarks" | "settings";
export type PageId = MarketingPageId | AppPageId;

export interface SitePage<Id extends string = PageId> {
  id: Id;
  href: string;
  label: string;
  /** show in the primary nav (marketing) / app nav (application) */
  nav?: boolean;
}

/** Every public page. Drives the sitemap, llms.txt and the .md/.txt variants.
 *  `nav: true` additionally puts it in the top bar. */
export const MARKETING_PAGES: SitePage<MarketingPageId>[] = [
  { id: "home", href: "/", label: "Home", nav: true },
  { id: "about", href: "/about", label: "About", nav: true },
  { id: "download", href: "/download", label: "Download", nav: true },
  { id: "privacy", href: "/privacy", label: "Privacy" },
];

/** The subset rendered as links in the primary nav. */
export const NAV_PAGES: SitePage<MarketingPageId>[] = MARKETING_PAGES.filter((p) => p.nav);

/** The /app product UI. Never indexed, never in the sitemap or text variants. */
export const APP_PAGES: SitePage<AppPageId>[] = [
  { id: "read", href: "/app/read", label: "Read", nav: true },
  { id: "bookmarks", href: "/app/bookmarks", label: "Bookmarks", nav: true },
  { id: "settings", href: "/app/settings", label: "Settings", nav: true },
];

/** Page-level metadata (title, description, canonical path). The single source
 *  of truth for <Seo> and, for marketing pages, the llms.txt index. */
export const PAGE_META: Record<PageId, { title: string; description: string; path: string }> = {
  home: {
    title: "EasyQuran · the Quran, made easy to read",
    description: "Read the Quran. Free, no ads, and fast — nothing in the way of the text.",
    path: "/",
  },
  about: {
    title: "About · EasyQuran",
    description: "What EasyQuran is, who it's for, and how it's built.",
    path: "/about",
  },
  download: {
    title: "Download · EasyQuran",
    description: "Get EasyQuran on your device, or just open it in the browser.",
    path: "/download",
  },
  privacy: {
    title: "Privacy · EasyQuran",
    description: "What EasyQuran collects, what it doesn't, and why.",
    path: "/privacy",
  },
  // ── application (noindex) ──────────────────────────────────────────────
  app: { title: "EasyQuran", description: "Your reading home.", path: "/app" },
  read: { title: "Read · EasyQuran", description: "Read the Quran.", path: "/app/read" },
  bookmarks: {
    title: "Bookmarks · EasyQuran",
    description: "Saved ayahs and places you left off.",
    path: "/app/bookmarks",
  },
  settings: {
    title: "Settings · EasyQuran",
    description: "Appearance, script, and reading preferences.",
    path: "/app/settings",
  },
};

export interface AccentDef {
  id: AccentId;
  label: string;
  hex: string;
}

/** The accent palette. Hex values are the dark-mode swatch colours. */
export const ACCENTS: AccentDef[] = [
  { id: "emerald", label: "Emerald", hex: "#4ade80" },
  { id: "gold", label: "Gold", hex: "#e3b341" },
  { id: "azure", label: "Azure", hex: "#6fb0e8" },
  { id: "plum", label: "Plum", hex: "#c08cff" },
];

export const DEFAULTS = {
  theme: "dark",
  accent: "emerald",
} satisfies Record<string, string>;
