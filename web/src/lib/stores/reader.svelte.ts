/* ════════════════════════════════════════════════════════════════════════
   reader.svelte.ts — the reading experience state (composition root).

   The reader was a single 387-line class combining transient UI state, durable
   settings, annotations, verse caching, navigation, persistence, and browser
   sharing. It is now composed from cohesive facets, each in its own module:
     • reader-core          — shared reactive state + SvelteMap + nav token
     • reader-persistence   — durable persistence + debounced note writes
     • reader-session       — query / browse / open-note / navigation
     • reader-settings      — Arabic font size + reading mode
     • annotations          — bookmarks / notes / last-read
     • verse-cache          — SvelteMap + Worker refresh coordination
     • reader-share         — copy / share side effects + share-text

   The public contract is the flat `ReaderApi` surface below — it is preserved
   EXACTLY (member names + behavior) so every consumer (SurahReader, VerseRow,
   Sidebar, Results, the (application)/app pages) keeps working. The singleton
   import path is stable: `import { reader } from "$lib/stores/reader.svelte"`.

   Removed (verified zero consumers): the `current`/`surah`/`surahCount`/
   `fontSize`/`bookmarkList`/`bookmarkCount` getters and the dead bookmark
   ternary. The persisted `current` FIELD is retained (used internally by
   load/persist/setCurrent/openVerse). See docs/svelte-improvements.md §4.B/§5.
   ════════════════════════════════════════════════════════════════════════ */

import type { VerseKey } from "$lib/data/quran";
import { createAnnotations } from "./annotations.svelte";
import { type BrowseMode, type ReaderMode, createReaderCore } from "./reader-core.svelte";
import { createReaderPersistence } from "./reader-persistence.svelte";
import { createReaderSession } from "./reader-session.svelte";
import { createReaderSettings } from "./reader-settings.svelte";
import { createReaderShare } from "./reader-share.svelte";
import { createVerseCache } from "./verse-cache.svelte";

export type { BrowseMode, ReaderMode } from "./reader-core.svelte";

/** The preserved flat public API of the reader singleton. */
export interface ReaderApi {
  // lifecycle
  hydrate(): void;
  dispose(): void;
  // search
  readonly query: string;
  readonly hasQuery: boolean;
  setQuery(v: string): void;
  clearQuery(): void;
  // sidebar browse
  readonly browseMode: BrowseMode;
  readonly browseSurah: boolean;
  readonly browseAyah: boolean;
  readonly browseJuz: boolean;
  readonly browsePage: boolean;
  setBrowse(browse: BrowseMode): void;
  // open note panel (UI)
  readonly openNote: VerseKey | null;
  toggleNote(key: VerseKey): void;
  // navigation
  setCurrent(num: number): void;
  openVerse(num: number, n: number): void;
  // settings
  readonly arabicSizePx: string;
  bigger(): void;
  smaller(): void;
  readonly mode: ReaderMode;
  readonly isVerseMode: boolean;
  readonly isReadingMode: boolean;
  setMode(mode: ReaderMode): void;
  // annotations
  isBookmarked(key: VerseKey): boolean;
  toggleBookmark(key: VerseKey): void;
  getNote(key: VerseKey): string;
  setNote(key: VerseKey, v: string): void;
  readonly lastRead: { num: number; n: number } | null;
  readonly hasLastRead: boolean;
  readonly lastReadRef: string;
  // verse cache
  versesFor(num: number): string[];
  seedSurah(num: number, verses: string[]): void;
  refreshFromWorker(num: number): Promise<void>;
  // copy & share
  copyVerse(key: VerseKey): Promise<boolean>;
  shareVerse(key: VerseKey): Promise<"shared" | "copied" | "failed">;
}

/**
 * Construct a fully-wired, isolated reader instance. The singleton `reader`
 * below is one such instance; tests (and any future subtree-scoped reader)
 * can create their own.
 */
export function createReader(): ReaderApi {
  const core = createReaderCore();
  const persistence = createReaderPersistence(core);
  const session = createReaderSession(core, persistence);
  const settings = createReaderSettings(core, persistence);
  const annotations = createAnnotations(core, persistence);
  const verseCache = createVerseCache(core);
  const share = createReaderShare(core);

  return {
    // lifecycle
    hydrate: () => persistence.hydrate(),
    dispose: () => persistence.dispose(),

    // search
    get query() {
      return session.query;
    },
    get hasQuery() {
      return session.hasQuery;
    },
    setQuery: (v: string) => session.setQuery(v),
    clearQuery: () => session.clearQuery(),

    // sidebar browse
    get browseMode() {
      return session.browseMode;
    },
    get browseSurah() {
      return session.browseSurah;
    },
    get browseAyah() {
      return session.browseAyah;
    },
    get browseJuz() {
      return session.browseJuz;
    },
    get browsePage() {
      return session.browsePage;
    },
    setBrowse: (browse: BrowseMode) => session.setBrowse(browse),

    // open note panel
    get openNote() {
      return session.openNote;
    },
    toggleNote: (key: VerseKey) => session.toggleNote(key),

    // navigation
    setCurrent: (num: number) => session.setCurrent(num),
    openVerse: (num: number, n: number) => session.openVerse(num, n),

    // settings
    get arabicSizePx() {
      return settings.arabicSizePx;
    },
    bigger: () => settings.bigger(),
    smaller: () => settings.smaller(),
    get mode() {
      return settings.mode;
    },
    get isVerseMode() {
      return settings.isVerseMode;
    },
    get isReadingMode() {
      return settings.isReadingMode;
    },
    setMode: (mode: ReaderMode) => settings.setMode(mode),

    // annotations
    isBookmarked: (key: VerseKey) => annotations.isBookmarked(key),
    toggleBookmark: (key: VerseKey) => annotations.toggleBookmark(key),
    getNote: (key: VerseKey) => annotations.getNote(key),
    setNote: (key: VerseKey, v: string) => annotations.setNote(key, v),
    get lastRead() {
      return annotations.lastRead;
    },
    get hasLastRead() {
      return annotations.hasLastRead;
    },
    get lastReadRef() {
      return annotations.lastReadRef;
    },

    // verse cache
    versesFor: (num: number) => verseCache.versesFor(num),
    seedSurah: (num: number, verses: string[]) => verseCache.seedSurah(num, verses),
    refreshFromWorker: (num: number) => verseCache.refreshFromWorker(num),

    // copy & share
    copyVerse: (key: VerseKey) => share.copyVerse(key),
    shareVerse: (key: VerseKey) => share.shareVerse(key),
  };
}

/** The application-wide reader singleton. Process-wide in the browser; on the
 *  server it renders from defaults and hydrates after mount. */
export const reader: ReaderApi = createReader();
