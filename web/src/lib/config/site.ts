export const SITE = {
  name: "EasyQuran",
  domain: "easyquran.fyi",
  url: "https://easyquran.fyi",
  github: "https://github.com/hmziqrs",
  maker: "oxlabs",
  makerUrl: "https://oxlabs.dev",
  owner: "hmziq.rs",
  ownerUrl: "https://hmziq.rs",
  contactEmail: "hmziqrs@gmail.com",
  tanzilUrl: "https://tanzil.net",
} as const;

import { dev } from "$app/environment";
import { env } from "$env/dynamic/public";
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

// ── Design-system palette + appearance model (docs/design-system.md §4, §25, §59) ──────────
// `palette` (sacred/ink/sepia/sapphire) and `mode` (light/dark/system) are separate settings.
// The legacy theme/surface/accent prefs above stay exported because settings-document.ts and the
// i18n copy resolvers still consume them; they are synced from the new fields, never the reverse.

/** Palette attribute values — must match the `[data-palette="…"]` selectors in layout.css. */
export type PaletteId = "sacred" | "ink" | "sepia" | "sapphire";

/** Appearance setting (§25): the tri-state the user picks; `system` resolves via matchMedia. */
export type AppearanceMode = "light" | "dark" | "system";

export const APPEARANCE_MODES: readonly AppearanceMode[] = ["light", "dark", "system"] as const;

export interface PaletteDef {
  id: PaletteId;
  /**
   * Ground preview swatches (plan 00 D1). Under the shared neutral ground every palette's
   * ground is identical, so these are no longer what the picker renders — kept in sync with
   * layout.css for any diagnostic use. The picker draws `accentHex`.
   */
  lightHex: string;
  darkHex: string;
  /** Accent preview swatch per mode — what the appearance picker actually shows (plan 06). */
  accentHex: { light: string; dark: string };
}

export const PALETTES: PaletteDef[] = [
  {
    id: "sacred",
    lightHex: "#f8f8f8",
    darkHex: "#0e0e0e",
    accentHex: { light: "#1a5cdf", dark: "#1957d2" },
  },
  {
    id: "ink",
    lightHex: "#f8f8f8",
    darkHex: "#0e0e0e",
    accentHex: { light: "#161616", dark: "#f3f3f3" },
  },
  {
    id: "sepia",
    lightHex: "#f8f8f8",
    darkHex: "#0e0e0e",
    accentHex: { light: "#c7007c", dark: "#b90073" },
  },
  {
    id: "sapphire",
    lightHex: "#f8f8f8",
    darkHex: "#0e0e0e",
    accentHex: { light: "#00864e", dark: "#007a49" },
  },
];

/** Back-compat migration for stored `surface` prefs → the new palette ids. */
export const SURFACE_TO_PALETTE = {
  ink: "ink",
  paper: "sepia",
  slate: "sapphire",
  mocha: "sacred",
  contrast: "ink",
} as const satisfies Record<SurfaceId, PaletteId>;

/** Reverse mapping so the legacy `surface` field (still decoded by settings-document.ts) tracks the palette. */
export const PALETTE_TO_SURFACE = {
  sacred: "mocha",
  ink: "ink",
  sepia: "paper",
  sapphire: "slate",
} as const satisfies Record<PaletteId, SurfaceId>;

export const DEFAULT_PALETTE: PaletteId = "sacred";
export const DEFAULT_MODE: AppearanceMode = "system";

export function paletteDef(id: PaletteId): PaletteDef {
  // SAFETY: PALETTES is a closed literal with exactly one entry per PaletteId.
  return PALETTES.find((p) => p.id === id) as PaletteDef;
}

export interface AccentDef {
  id: AccentId;
  hex: string;
}

export const ACCENTS: AccentDef[] = [
  { id: "emerald", hex: "#3fbfa6" },
  { id: "gold", hex: "#d9af6a" },
  { id: "azure", hex: "#6fb0e8" },
  { id: "plum", hex: "#c08cff" },
];

export interface SurfaceDef {
  id: SurfaceId;
  darkHex: string;
  lightHex: string;
}

export const SURFACES: SurfaceDef[] = [
  {
    id: "ink",
    darkHex: "#0a0a0a",
    lightHex: "#ffffff",
  },
  {
    id: "paper",
    darkHex: "#151210",
    lightHex: "#faf6ef",
  },
  {
    id: "slate",
    darkHex: "#0d1117",
    lightHex: "#f6f8fa",
  },
  {
    id: "mocha",
    darkHex: "#17110e",
    lightHex: "#f7f1ea",
  },
  {
    id: "contrast",
    darkHex: "#000000",
    lightHex: "#ffffff",
  },
];

export interface ThemeDefaults {
  theme: ThemeMode;
  accent: AccentId;
  surface: SurfaceId;
}

export const DEFAULTS: ThemeDefaults = {
  theme: "dark",
  accent: "emerald",
  surface: "ink",
};
