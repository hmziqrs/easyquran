export const SITE = {
  name: "EasyQuran",
  domain: "easyquran.fyi",
  url: "https://easyquran.fyi",
  github: "https://github.com/hmziqrs",
  tagline: "the Quran, made easy to read",
  footerBlurb: "A free Qur'an reader — growing into hadith, audio, deeds and native apps.",
  maker: "oxlabs",
  makerUrl: "https://oxlabs.dev",
  owner: "hmziq.rs",
  ownerUrl: "https://hmziq.rs",
} as const;

import { env } from "$env/dynamic/public";
import { dev } from "$app/environment";
import {
  QuranDataEnvironment,
  resolveQuranArtifactBase,
  resolveQuranDataEnvironment,
} from "$lib/quran/environment";
import { registeredSourceProfiles } from "$lib/quran/view/source-profiles";

const PUBLIC_API_BASE = (env.PUBLIC_QURAN_API_BASE ?? "").replace(/\/+$/, "");
const QURAN_DATA_ENVIRONMENT = resolveQuranDataEnvironment(
  env.PUBLIC_ENV,
  dev ? QuranDataEnvironment.Local : QuranDataEnvironment.Production,
);
const QURAN_ARTIFACT_BASE = resolveQuranArtifactBase(QURAN_DATA_ENVIRONMENT);

const QURAN_ARTIFACTS = Object.freeze(
  registeredSourceProfiles().map((profile) => ({
    id: profile.sourceId,
    sizeBytes: profile.artifact.sizeBytes,
    downloadUrl: `${QURAN_ARTIFACT_BASE}/${profile.artifact.r2Path}`,
  })),
);

export const QURAN = {
  apiBase: PUBLIC_API_BASE,
  dataEnvironment: QURAN_DATA_ENVIRONMENT,
  artifactBase: QURAN_ARTIFACT_BASE,
  scripts: QURAN_ARTIFACTS,
} as const;

export type ThemeMode = "dark" | "light";
export type AccentId = "emerald" | "gold" | "azure" | "plum";
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
