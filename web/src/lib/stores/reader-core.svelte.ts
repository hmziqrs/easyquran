import { SvelteMap } from "svelte/reactivity";
import type { VerseKey } from "$lib/data/quran";

export type BrowseMode = "surah" | "ayah" | "juz" | "page";
export type ReaderMode = "verse" | "reading";

export interface Persisted {
  v: number;
  current: number;
  fontSize: number;
  mode: ReaderMode;
  bookmarks: Record<VerseKey, boolean>;
  notes: Record<VerseKey, string>;
  lastRead: { num: number; n: number } | null;
}

export interface ReaderState extends Persisted {
  query: string;
  browse: BrowseMode;
  openNote: VerseKey | null;
}

export const READER_SCHEMA_VERSION = 1;

export const ARABIC_FONT_MIN = 22;
export const ARABIC_FONT_MAX = 56;
export const ARABIC_FONT_STEP = 3;

export const NOTE_PERSIST_DEBOUNCE_MS = 400;

const READER_MODES: readonly ReaderMode[] = ["verse", "reading"];

export const READER_DEFAULTS: ReaderState = {
  v: READER_SCHEMA_VERSION,
  current: 1,
  fontSize: 33,
  mode: "verse",
  bookmarks: {},
  notes: {},
  lastRead: null,
  query: "",
  browse: "surah",
  openNote: null,
};

export interface NavToken {
  readonly token: number;
  bump(): void;
}

export interface ReaderCore {
  readonly s: ReaderState;
  readonly versesBySurah: SvelteMap<number, string[]>;
  readonly nav: NavToken;
}

export function createReaderCore(): ReaderCore {
  const s = $state<ReaderState>({ ...READER_DEFAULTS });
  const versesBySurah = new SvelteMap<number, string[]>();
  let navToken = 0;
  const nav: NavToken = {
    get token() {
      return navToken;
    },
    bump() {
      navToken += 1;
    },
  };
  return { s, versesBySurah, nav };
}

export const SURAH_COUNT = 114;
export const READER_MODE_VALUES: readonly ReaderMode[] = READER_MODES;
