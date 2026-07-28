/* ════════════════════════════════════════════════════════════════════════
   site.ts — the single source of truth for the EasyQuran site.
   Nav, footer, accent palette, and page metadata all derive from the values
   declared here. Nothing about the site's structure is duplicated across
   components.

   ⚠ Placeholders: `domain` / `url` / the social handles below are stand-ins.
   Set them once here and the Seo component, sitemap.xml, llms.txt and
   robots.txt all follow.
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

export type PageId = "home" | "about";

export interface NavPage {
  id: PageId;
  href: string;
  label: string;
}

/** Primary navigation (Home is rendered as the brand, so it's listed but
 *  not always shown as a text link). */
export const NAV_PAGES: NavPage[] = [
  { id: "home", href: "/", label: "Home" },
  { id: "about", href: "/about", label: "About" },
];

export type PageSlug = PageId;

/** Page-level SEO metadata (title, description, canonical path). The single
 *  source of truth for the <Seo> component and the llms.txt endpoint. */
export const PAGE_META: Record<PageSlug, { title: string; description: string; path: string }> = {
  home: {
    title: "EasyQuran · the Quran, made easy to read",
    description: "A calm, fast, distraction-free way to read the Quran.",
    path: "/",
  },
  about: {
    title: "About · EasyQuran",
    description: "What EasyQuran is, who it's for, and how it's built.",
    path: "/about",
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
