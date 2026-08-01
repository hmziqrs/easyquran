import type { VerseKey } from "$lib/data/quran";
import { createAnnotations } from "./annotations.svelte";
import { type BrowseMode, type ReaderMode, createReaderCore } from "./reader-core.svelte";
import { createReaderPersistence } from "./reader-persistence.svelte";
import { createReaderSession } from "./reader-session.svelte";
import { createReaderSettings } from "./reader-settings.svelte";
import { createReaderShare } from "./reader-share.svelte";
import { createVerseCache } from "./verse-cache.svelte";

export type { BrowseMode, ReaderMode } from "./reader-core.svelte";

export interface ReaderApi {
  hydrate(): void;
  dispose(): void;
  readonly query: string;
  readonly hasQuery: boolean;
  setQuery(v: string): void;
  clearQuery(): void;
  readonly browseMode: BrowseMode;
  readonly browseSurah: boolean;
  readonly browseAyah: boolean;
  readonly browseJuz: boolean;
  readonly browsePage: boolean;
  setBrowse(browse: BrowseMode): void;
  readonly openNote: VerseKey | null;
  toggleNote(key: VerseKey): void;
  setCurrent(num: number): void;
  openVerse(num: number, n: number): void;
  readonly arabicSizePx: string;
  bigger(): void;
  smaller(): void;
  readonly mode: ReaderMode;
  readonly isVerseMode: boolean;
  readonly isReadingMode: boolean;
  setMode(mode: ReaderMode): void;
  isBookmarked(key: VerseKey): boolean;
  toggleBookmark(key: VerseKey): void;
  getNote(key: VerseKey): string;
  setNote(key: VerseKey, v: string): void;
  readonly lastRead: { num: number; n: number } | null;
  readonly hasLastRead: boolean;
  readonly lastReadRef: string;
  versesFor(num: number): string[];
  seedSurah(num: number, verses: string[]): void;
  refreshFromWorker(num: number): Promise<void>;
  copyVerse(key: VerseKey): Promise<boolean>;
  shareVerse(key: VerseKey): Promise<"shared" | "copied" | "failed">;
}

export function createReader(): ReaderApi {
  const core = createReaderCore();
  const persistence = createReaderPersistence(core);
  const session = createReaderSession(core, persistence);
  const settings = createReaderSettings(core, persistence);
  const annotations = createAnnotations(core, persistence);
  const verseCache = createVerseCache(core);
  const share = createReaderShare(core);

  return {
    hydrate: () => persistence.hydrate(),
    dispose: () => persistence.dispose(),

    get query() {
      return session.query;
    },
    get hasQuery() {
      return session.hasQuery;
    },
    setQuery: (v: string) => session.setQuery(v),
    clearQuery: () => session.clearQuery(),

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

    get openNote() {
      return session.openNote;
    },
    toggleNote: (key: VerseKey) => session.toggleNote(key),

    setCurrent: (num: number) => session.setCurrent(num),
    openVerse: (num: number, n: number) => session.openVerse(num, n),

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

    versesFor: (num: number) => verseCache.versesFor(num),
    seedSurah: (num: number, verses: string[]) => verseCache.seedSurah(num, verses),
    refreshFromWorker: (num: number) => verseCache.refreshFromWorker(num),

    copyVerse: (key: VerseKey) => share.copyVerse(key),
    shareVerse: (key: VerseKey) => share.shareVerse(key),
  };
}

export const reader: ReaderApi = createReader();
