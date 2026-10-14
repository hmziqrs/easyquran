import { SvelteMap } from "svelte/reactivity";

import {
  DEFAULT_ARABIC_FONT,
  TRANSLATION_FAMILIES,
  type ArabicFontId,
  type TranslationFamily,
} from "#lib/config/reader-fonts.js";
import type { ReaderPositionState } from "#lib/data/mushaf-divisions.js";
import { QURAN_SOURCE_IDS, QuranSourceId } from "#lib/data/quran-types.js";
import type { VerseKey } from "#lib/data/quran.js";

export const BrowseMode = {
  Surah: "surah",
  Ayah: "ayah",
  Juz: "juz",
  Page: "page",
} as const;
export type BrowseMode = (typeof BrowseMode)[keyof typeof BrowseMode];
export type ReaderMode = "verse" | "reading";

export interface LastReadAnchor {
  verseKey: string;
  localPage: number;
  ratio: number;
}

export interface Persisted {
  v: number;
  current: number;
  fontSize: number;
  arabicFont: ArabicFontId;
  /** Mushaf script variant for Arabic reading (source id of the Arabic corpus). */
  arabicScript: QuranSourceId;
  translationSize: number;
  /** Translation typeface family (TRX-10). v5 persists it; earlier blobs default to "sans". */
  translationFamily: TranslationFamily;
  mode: ReaderMode;
  /**
   * Legacy v≤4 field — migration input only. v5 stopped writing it: notes live under
   * the per-owner key `easyquran.notes.{owner}` (EC-17), never inside the blob.
   */
  notes: Record<VerseKey, string>;
}

export interface ReaderState extends Persisted {
  query: string;
  browse: BrowseMode;
  openNote: VerseKey | null;
  pendingAnchor: LastReadAnchor | null;
  /** Live mushaf position for the sticky indicator. Transient — never persisted. */
  position: ReaderPositionState | null;
}

// v4 adds `arabicScript` (Uthmani/Simple-clean/IndoPak/Tajweed mushaf variant).
// v5 adds `translationFamily` and moves notes out to `easyquran.notes.{owner}`.
// v6 drops position + bookmark fields: reading threads (#lib/sessions) and
// bookmarks (#lib/bookmarks) own them now. Decode reads supported fields only.
export const READER_SCHEMA_VERSION = 6;

export const ARABIC_SCRIPT_VALUES: readonly QuranSourceId[] = QURAN_SOURCE_IDS;

export const TRANSLATION_FAMILY_VALUES: readonly TranslationFamily[] = TRANSLATION_FAMILIES.map(
  (f) => f.id,
);

export const ARABIC_FONT_MIN = 22;
export const ARABIC_FONT_MAX = 56;
export const ARABIC_FONT_STEP = 3;

export const TRANSLATION_FONT_MIN = 13;
export const TRANSLATION_FONT_MAX = 28;
export const TRANSLATION_FONT_STEP = 1;

export const NOTE_PERSIST_DEBOUNCE_MS = 400;

const READER_MODES: readonly ReaderMode[] = ["verse", "reading"];

export const READER_DEFAULTS: ReaderState = {
  v: READER_SCHEMA_VERSION,
  current: 1,
  fontSize: 33,
  arabicFont: DEFAULT_ARABIC_FONT,
  // Default mushaf = fully annotated Uthmani (quran.com word lineage): small-meem
  // pausals + waqf stop marks the plain Tanzil edition omits. Tanzil stays selectable.
  arabicScript: QuranSourceId.AnnotatedUthmani,
  translationSize: 17,
  translationFamily: "sans",
  mode: "verse",
  notes: {},
  pendingAnchor: null,
  query: "",
  browse: BrowseMode.Surah,
  openNote: null,
  position: null,
};

export interface NavToken {
  readonly token: number;
  bump(): void;
}

export interface ReaderCore {
  readonly s: ReaderState;
  readonly verseTextBySurah: Map<number, Map<number, string>>;
  readonly verseVersionBySurah: SvelteMap<number, number>;
  readonly nav: NavToken;
}

export function createReaderCore(): ReaderCore {
  const s = $state<ReaderState>({ ...READER_DEFAULTS });
  const verseTextBySurah = new Map<number, Map<number, string>>();
  const verseVersionBySurah = new SvelteMap<number, number>();
  let navToken = 0;
  const nav: NavToken = {
    get token() {
      return navToken;
    },
    bump() {
      navToken += 1;
    },
  };
  return { s, verseTextBySurah, verseVersionBySurah, nav };
}

export const SURAH_COUNT = 114;
export const READER_MODE_VALUES: readonly ReaderMode[] = READER_MODES;
