/* ════════════════════════════════════════════════════════════════════════
   site.ts — the single source of truth for the EasyQuran site.

   The route tree is split into two groups (see src/routes):
     • (marketing) — public, indexable pages. These drive the nav, the
       sitemap, llms.txt and the .md/.txt text variants.
     • (application) — the /app reader. Deliberately kept OUT of the
       sitemap, llms.txt and text variants, and marked noindex via <Seo>.

   ⚠ Placeholders: `domain` / `url` / the social handles below are stand-ins.
   Set them once here and <Seo>, sitemap.xml, llms.txt and robots.txt follow.
   ════════════════════════════════════════════════════════════════════════ */

export const SITE = {
  name: "EasyQuran",
  domain: "easyquran.fyi",
  url: "https://easyquran.fyi",
  github: "https://github.com/hmziqrs",
  tagline: "the Quran, made easy to read",
  /** one-line value prop used in the footer (distinct from the hero tagline). */
  footerBlurb: "A free Qur'an reader — growing into hadith, audio, deeds and native apps.",
  // Owner/maker contact (email + X) is sourced server-side from
  // hmziq.rs/me.json — see $lib/server/owner.ts — so it isn't hardcoded here.
  /** project maker + owner, credited in the footer and on the about page. */
  maker: "oxlabs",
  makerUrl: "https://oxlabs.dev",
  owner: "hmziq.rs",
  ownerUrl: "https://hmziq.rs",
} as const;

/* ════════════════════════════════════════════════════════════════════════
   QURAN — Quran content delivery config (docs/quran-web-delivery.md).

   The two Arabic SQLite files + the metadata XML are immutable and already
   published to R2 (r2.easyquran.fyi). `apiBase` points at the /quran/v1 Rust
   API; it is EMPTY today (the API is being built in parallel), so the manifest
   resolver (lib/quran/manifest.ts) falls back to the BAKED constants below —
   every data path works with no backend, then lights up the live API when
   `apiBase` is set and the host responds. Set it via PUBLIC_QURAN_API_BASE.
   ════════════════════════════════════════════════════════════════════════ */

import { env } from "$env/dynamic/public";
import { SEARCH_VERSION } from "$lib/quran/search/normalize";

/** Same-origin by default; override with a full URL when the API is elsewhere. */
const PUBLIC_API_BASE = (env.PUBLIC_QURAN_API_BASE ?? "").replace(/\/+$/, "");

export const QURAN = {
  apiBase: PUBLIC_API_BASE,
  r2Base: "https://r2.easyquran.fyi",
  /** BLAKE3(uthmani || simple-clean || xml)[0..16] (docs/quran-api.md §8.1).
   *  Baked until /quran/v1/version is live; the resolver overrides it then. */
  contentVersion: "32cc746d817cad9f",
  /** Bumped whenever the shared normalization rules change (docs §7). */
  searchVersion: SEARCH_VERSION,
  /** R2 paths mirror db/quran/tanzil (see translations/scripts/upload-sqlite.ts). */
  scripts: [
    {
      id: "uthmani" as const,
      sizeBytes: 1_593_344,
      sha256: "581cc5405831bc072fccd8db55cd1db72c5c5440c39bd975edbf03447efecf53",
      downloadUrl: "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite",
    },
    {
      id: "simple-clean" as const,
      sizeBytes: 929_792,
      sha256: "a0c52760d6660ac5be1de5c76bb10df7a839a3e8a87ecb0e636fe2ed45b2e4a3",
      downloadUrl: "https://r2.easyquran.fyi/tanzil/arabic/quran-simple-clean.sqlite",
    },
  ],
} as const;

export type ThemeMode = "dark" | "light";
export type AccentId = "emerald" | "gold" | "azure" | "plum";

export type MarketingPageId = "home" | "about" | "faq" | "contact" | "privacy" | "terms";
export type AppPageId = "app";
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
  { id: "faq", href: "/faq", label: "FAQ", nav: true },
  { id: "contact", href: "/contact", label: "Contact", nav: true },
  { id: "privacy", href: "/privacy", label: "Privacy" },
  { id: "terms", href: "/terms", label: "Terms" },
];

/** The links rendered in the primary nav. Read jumps into the (noindex) reader;
 *  the rest are marketing pages. */
export interface NavLink {
  href: string;
  label: string;
}
export const NAV_LINKS: NavLink[] = [
  { href: "/app", label: "Read" },
  { href: "/about", label: "About" },
  { href: "/faq", label: "FAQ" },
  { href: "/contact", label: "Contact" },
];

/** The /app reader. Never indexed, never in the sitemap or text variants. */
export const APP_PAGES: SitePage<AppPageId>[] = [{ id: "app", href: "/app", label: "Read" }];

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
  faq: {
    title: "FAQ · EasyQuran",
    description: "Questions about EasyQuran, answered plainly.",
    path: "/faq",
  },
  contact: {
    title: "Contact · EasyQuran",
    description: "Get in touch — bug reports, script corrections and feature ideas.",
    path: "/contact",
  },
  privacy: {
    title: "Privacy · EasyQuran",
    description: "What EasyQuran collects, what it doesn't, and why.",
    path: "/privacy",
  },
  terms: {
    title: "Terms · EasyQuran",
    description: "The terms under which EasyQuran is provided.",
    path: "/terms",
  },
  // ── application (noindex) ──────────────────────────────────────────────
  app: { title: "EasyQuran", description: "Read the Quran.", path: "/app" },
};

export interface AccentDef {
  id: AccentId;
  label: string;
  hex: string;
}

/** The accent palette. Hex values are the dark-mode swatch colours. */
export const ACCENTS: AccentDef[] = [
  { id: "emerald", label: "Teal", hex: "#3fbfa6" },
  { id: "gold", label: "Gold", hex: "#d9af6a" },
  { id: "azure", label: "Azure", hex: "#6fb0e8" },
  { id: "plum", label: "Plum", hex: "#c08cff" },
];

export const DEFAULTS: { theme: ThemeMode; accent: AccentId } = {
  theme: "dark",
  accent: "emerald",
};
