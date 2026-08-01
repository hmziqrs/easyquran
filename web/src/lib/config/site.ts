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
  footerBlurb: "A free Qur'an reader — growing into hadith, audio, deeds and native apps.",
  // Owner/maker contact (email + X) is sourced server-side from
  // hmziq.rs/me.json — see $lib/server/owner.ts — so it isn't hardcoded here.
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
import { registeredSourceProfiles } from "$lib/quran/view/source-profiles";

const PUBLIC_API_BASE = (env.PUBLIC_QURAN_API_BASE ?? "").replace(/\/+$/, "");
const QURAN_R2_BASE = "https://r2.easyquran.fyi";

/** Delivery metadata is derived from the same source registry used by SSG and the Worker. */
const QURAN_ARTIFACTS = Object.freeze(
  registeredSourceProfiles().map((profile) => ({
    id: profile.sourceId,
    sizeBytes: profile.artifact.sizeBytes,
    sha256: profile.artifact.sha256,
    downloadUrl: `${QURAN_R2_BASE}/${profile.artifact.r2Path}`,
  })),
);

export const QURAN = {
  apiBase: PUBLIC_API_BASE,
  r2Base: QURAN_R2_BASE,
  /** BLAKE3(uthmani || simple-clean || xml)[0..16] (docs/quran-api.md §8.1).
   *  Baked until /quran/v1/version is live; the resolver overrides it then. */
  contentVersion: "32cc746d817cad9f",
  /** Bumped whenever the shared normalization rules change (docs §7). */
  searchVersion: SEARCH_VERSION,
  /** R2 paths mirror db/quran/tanzil (see translations/scripts/upload-sqlite.ts). */
  scripts: QURAN_ARTIFACTS,
} as const;

export type ThemeMode = "dark" | "light";
export type AccentId = "emerald" | "gold" | "azure" | "plum";
/** Background/neutral family. Orthogonal to theme and accent — every surface
 *  defines both a dark and a light set of values in layout.css. */
export type SurfaceId = "ink" | "paper" | "slate" | "mocha" | "contrast";

export type MarketingPageId = "home" | "about" | "faq" | "contact" | "privacy" | "terms";
export type AppPageId = "app";
export type PageId = MarketingPageId | AppPageId;

export interface SitePage<Id extends string = PageId> {
  id: Id;
  href: string;
  label: string;
  nav?: boolean;
}

export const MARKETING_PAGES: SitePage<MarketingPageId>[] = [
  { id: "home", href: "/", label: "Home", nav: true },
  { id: "about", href: "/about", label: "About", nav: true },
  { id: "faq", href: "/faq", label: "FAQ", nav: true },
  { id: "contact", href: "/contact", label: "Contact", nav: true },
  { id: "privacy", href: "/privacy", label: "Privacy" },
  { id: "terms", href: "/terms", label: "Terms" },
];

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
  app: { title: "EasyQuran", description: "Read the Quran.", path: "/app" },
};

export interface AccentDef {
  id: AccentId;
  label: string;
  hex: string;
}

export const ACCENTS: AccentDef[] = [
  { id: "emerald", label: "Teal", hex: "#3fbfa6" },
  { id: "gold", label: "Gold", hex: "#d9af6a" },
  { id: "azure", label: "Azure", hex: "#6fb0e8" },
  { id: "plum", label: "Plum", hex: "#c08cff" },
];

export interface SurfaceDef {
  id: SurfaceId;
  label: string;
  note: string;
  darkHex: string;
  lightHex: string;
}

/** The background families. Values live in layout.css under
 *  [data-theme][data-surface]; these entries only drive the picker UI. */
export const SURFACES: SurfaceDef[] = [
  {
    id: "ink",
    label: "Ink",
    note: "Neutral near-black / paper white. The default.",
    darkHex: "#0a0a0a",
    lightHex: "#ffffff",
  },
  {
    id: "paper",
    label: "Paper",
    note: "Warm sepia — closest to a printed mushaf.",
    darkHex: "#151210",
    lightHex: "#faf6ef",
  },
  {
    id: "slate",
    label: "Slate",
    note: "Cool blue-grey, slightly softer contrast.",
    darkHex: "#0d1117",
    lightHex: "#f6f8fa",
  },
  {
    id: "mocha",
    label: "Mocha",
    note: "Deep espresso browns, low glare at night.",
    darkHex: "#17110e",
    lightHex: "#f7f1ea",
  },
  {
    id: "contrast",
    label: "Contrast",
    note: "Pure black / pure white — maximum legibility.",
    darkHex: "#000000",
    lightHex: "#ffffff",
  },
];

export const DEFAULTS: { theme: ThemeMode; accent: AccentId; surface: SurfaceId } = {
  theme: "dark",
  accent: "emerald",
  surface: "ink",
};
