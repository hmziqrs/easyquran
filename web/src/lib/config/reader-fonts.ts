import { QuranDataEnvironment } from "#lib/quran/environment.js";

export type ArabicFontId =
  | "amiri"
  | "scheherazade-new"
  | "noto-naskh-arabic"
  | "kfgqpc-hafs"
  | "kfgqpc-taha-v1"
  | "kfgqpc-taha-v1-bold";
export type TranslationFamily = "sans" | "serif";

export interface ArabicFontDef {
  readonly id: ArabicFontId;
  readonly family: string;
  readonly stack: string;
  readonly file?: () => Promise<string>;
  /** Font container hint for the FontFace loader; woff2 for packaged fonts. */
  readonly format?: "woff2" | "truetype";
  /**
   * False when the face only serves a production-disabled script family
   * (PRODUCTION_DISABLED_SOURCES in #lib/quran/script-availability.js).
   * `enabledArabicFontIds` drops those faces in production; local keeps them.
   */
  readonly productionEnabled: boolean;
}

export interface TranslationFamilyDef {
  readonly id: TranslationFamily;
  readonly stack: string;
}

export const ARABIC_FONT_IDS: readonly ArabicFontId[] = [
  "amiri",
  "scheherazade-new",
  "noto-naskh-arabic",
  "kfgqpc-hafs",
  "kfgqpc-taha-v1",
  "kfgqpc-taha-v1-bold",
];

// Quran.com's ayah-column face is KFGQPC Uthmanic Hafs (they serve Ver18; our
// bundled copy is the newer Ver22), so mushaf parity defaults to it — it also
// covers the annotated edition's full annotation block and composes U+06DD
// without needing the ornament fallback.
export const DEFAULT_ARABIC_FONT: ArabicFontId = "kfgqpc-hafs";
export const DEFAULT_TRANSLATION_FAMILY: TranslationFamily = "sans";

const ARABIC_FALLBACKS = `"Traditional Arabic", "Geeza Pro", serif`;

// Script-gated faces. A face belongs to a production-disabled script family when its
// glyph coverage is built for that family alone; such faces set `productionEnabled: false`
// and are dropped by `enabledArabicFontIds` in production. Today every selectable face is
// enabled. Known future entries, listed so they slot in here and not into the script pills:
//   - `indopak-reader-compat-*` — OFL compatibility faces loaded only from the
//     production-disabled IndoPak path (docs/indopak.md).
//   - `indopak-nastaleeq-hanafi-v4.2.2` — Hanafi nastaleeq face from the unmerged
//     indopak branch; the known restricted face that slots in when that branch lands.
export const ARABIC_FONTS: readonly ArabicFontDef[] = [
  {
    id: "amiri",
    family: "Amiri",
    stack: `"Amiri", "Scheherazade New", ${ARABIC_FALLBACKS}`,
    productionEnabled: true,
  },
  {
    id: "scheherazade-new",
    family: "Scheherazade New",
    stack: `"Scheherazade New", "Amiri", ${ARABIC_FALLBACKS}`,
    file: () =>
      import("@fontsource/scheherazade-new/files/scheherazade-new-arabic-400-normal.woff2?url").then(
        (m) => m.default,
      ),
    productionEnabled: true,
  },
  {
    id: "noto-naskh-arabic",
    family: "Noto Naskh Arabic",
    stack: `"Noto Naskh Arabic", "Amiri", ${ARABIC_FALLBACKS}`,
    file: () =>
      import("@fontsource/noto-naskh-arabic/files/noto-naskh-arabic-arabic-400-normal.woff2?url").then(
        (m) => m.default,
      ),
    productionEnabled: true,
  },
  {
    // King Fahad Complex digital mushaf fonts (QPC, free to redistribute),
    // self-hosted in static/fonts and lazy-loaded on selection.
    id: "kfgqpc-hafs",
    family: "KFGQPC Uthmanic Hafs",
    stack: `"KFGQPC Uthmanic Hafs", "Amiri", ${ARABIC_FALLBACKS}`,
    file: () => Promise.resolve("/fonts/uthmanic-hafs-v22.ttf"),
    format: "truetype",
    productionEnabled: true,
  },
  {
    id: "kfgqpc-taha-v1",
    family: "KFGQPC Uthman Taha Naskh v1",
    stack: `"KFGQPC Uthman Taha Naskh v1", "Amiri", ${ARABIC_FALLBACKS}`,
    file: () => Promise.resolve("/fonts/uthman-taha-naskh-v1.woff2"),
    productionEnabled: true,
  },
  {
    id: "kfgqpc-taha-v1-bold",
    family: "KFGQPC Uthman Taha Naskh v1 Bold",
    stack: `"KFGQPC Uthman Taha Naskh v1 Bold", "Amiri", ${ARABIC_FALLBACKS}`,
    file: () => Promise.resolve("/fonts/uthman-taha-naskh-v1-bold.woff2"),
    productionEnabled: true,
  },
];

export const TRANSLATION_FAMILIES: readonly TranslationFamilyDef[] = [
  { id: "sans", stack: "var(--font-sans)" },
  { id: "serif", stack: `ui-serif, Georgia, "Times New Roman", serif` },
];

export function arabicFontDef(id: ArabicFontId): ArabicFontDef {
  const def = ARABIC_FONTS.find((f) => f.id === id);
  return def ?? ARABIC_FONTS[0]!;
}

export function arabicFontStack(id: ArabicFontId): string {
  return arabicFontDef(id).stack;
}

/**
 * Font ids selectable in the given environment. Production drops faces whose
 * `productionEnabled` is false (script-gated families, see the registry comment);
 * local keeps the full list.
 */
export function enabledArabicFontIds(environment: QuranDataEnvironment): readonly ArabicFontId[] {
  if (environment !== QuranDataEnvironment.Production) return ARABIC_FONT_IDS;
  return ARABIC_FONTS.filter((font) => font.productionEnabled).map((font) => font.id);
}

export function translationFamilyStack(id: TranslationFamily): string {
  const def = TRANSLATION_FAMILIES.find((f) => f.id === id);
  return def?.stack ?? "var(--font-sans)";
}
