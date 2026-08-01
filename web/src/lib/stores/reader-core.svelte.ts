/* ════════════════════════════════════════════════════════════════════════
   reader-core.svelte.ts — the shared reactive core of the reader.

   Holds the durable+transient state record (`$state`), the per-open-surah
   synchronous verse cache (`SvelteMap`), and the monotonic navigation token
   that guards stale Worker refreshes. Every reader facet (session, settings,
   annotations, verse-cache, share) is built over this core by `createReader`.

   This module owns NO side effects and NO persistence — those live in the
   facets and in reader-persistence. That keeps it cheap to construct for
   isolated unit tests (createReaderCore()) and free of localStorage/window
   dependencies.
   ════════════════════════════════════════════════════════════════════════ */

import { SvelteMap } from "svelte/reactivity";
import { SURAHS } from "$lib/data/quran";
import type { VerseKey } from "$lib/data/quran";

export type BrowseMode = "surah" | "ayah" | "juz" | "page";
export type ReaderMode = "verse" | "reading";

/** Durable slice persisted to localStorage under `easyquran.reader`. */
export interface Persisted {
  /** Schema version — bump when the persisted shape changes (see isFutureSchema). */
  v: number;
  /** Currently-selected surah (1..114). Internal-only field; no public getter. */
  current: number;
  /** Arabic font size in px. Reader-specific (NOT in the global appearance store). */
  fontSize: number;
  mode: ReaderMode;
  bookmarks: Record<VerseKey, boolean>;
  notes: Record<VerseKey, string>;
  /** Last-read position (for the "continue reading" entry point). */
  lastRead: { num: number; n: number } | null;
}

export interface ReaderState extends Persisted {
  /** Sidebar search box contents (resets on navigation). */
  query: string;
  browse: BrowseMode;
  openNote: VerseKey | null;
}

export const READER_SCHEMA_VERSION = 1;

/** Arabic font-size bounds and step (doc §5: named constants, not magic numbers). */
export const ARABIC_FONT_MIN = 22;
export const ARABIC_FONT_MAX = 56;
export const ARABIC_FONT_STEP = 3;

/**
 * Trailing debounce window for note persistence. localStorage is synchronous;
 * without debouncing, a keystroke in a note blocks the main thread on every
 * press. Discrete clicks (bookmark/font/mode) persist immediately.
 */
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

/** The navigation guard counter — bumped on every navigation so a stale Worker
 *  response for a previously-open surah can never clobber the current one. */
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

/** Re-exported for the persistence decoder's range checks. */
export const SURAH_COUNT = SURAHS.length;
export const READER_MODE_VALUES: readonly ReaderMode[] = READER_MODES;
