import type { ArabicFontId, TranslationFamily } from "#lib/config/reader-fonts.js";
import type { ReaderPositionState } from "#lib/data/mushaf-divisions.js";
import type { QuranSourceId } from "#lib/data/quran-types.js";
import type { VerseKey } from "#lib/data/quran.js";

import { createAnnotations } from "./annotations.svelte";
import {
  type BrowseMode,
  type LastReadAnchor,
  type ReaderMode,
  createReaderCore,
} from "./reader-core.svelte";
import { createReaderPersistence } from "./reader-persistence.svelte";
import { createReaderSession } from "./reader-session.svelte";
import { createReaderSettings } from "./reader-settings.svelte";
import { createReaderShare, type ShareResult, type ShareVerse } from "./reader-share.svelte";
import { createVerseCache } from "./verse-cache.svelte";

export { BrowseMode } from "./reader-core.svelte";
export type { ReaderMode } from "./reader-core.svelte";

export interface ReaderApi {
  hydrate(modeOverride?: ReaderMode): void;
  dispose(): void;
  readonly browseMode: BrowseMode;
  readonly browseSurah: boolean;
  readonly browseAyah: boolean;
  readonly browseJuz: boolean;
  readonly browsePage: boolean;
  setBrowse(browse: BrowseMode): void;
  readonly current: number;
  readonly openNote: VerseKey | null;
  toggleNote(key: VerseKey): void;
  setCurrent(num: number): void;
  /** Jump to a verse: the reader route resolves the reading thread. */
  openVerse(num: number): void;
  markRead(num: number, n: number, sourceId?: string): void;
  readonly arabicSizePx: string;
  bigger(): void;
  smaller(): void;
  readonly arabicFont: ArabicFontId;
  setArabicFont(id: ArabicFontId): void;
  readonly arabicScript: QuranSourceId;
  setArabicScript(id: QuranSourceId): void;
  readonly translationSizePx: string;
  growTranslation(): void;
  shrinkTranslation(): void;
  readonly translationFamily: TranslationFamily;
  setTranslationFamily(family: TranslationFamily): void;
  readonly mode: ReaderMode;
  readonly isVerseMode: boolean;
  readonly isReadingMode: boolean;
  setMode(mode: ReaderMode): void;
  /** Live mushaf position (sticky indicator). Transient — null until a reader view writes it. */
  readonly position: ReaderPositionState | null;
  setPosition(position: ReaderPositionState | null): void;
  getNote(key: VerseKey): string;
  setNote(key: VerseKey, v: string): void;
  readonly lastRead: { num: number; n: number; sourceId?: string } | null;
  readonly hasLastRead: boolean;
  readonly lastReadRef: string;
  readonly lastReadAnchor: LastReadAnchor | null;
  setLastReadAnchor(anchor: LastReadAnchor): void;
  readonly pendingAnchor: LastReadAnchor | null;
  setPendingAnchor(anchor: LastReadAnchor | null): void;
  consumePendingAnchor(): LastReadAnchor | null;
  progressFor(num: number): number | null;
  versesFor(num: number): string[];
  seedAyahs(ayahs: readonly { key: VerseKey; text: string }[]): void;
  refreshFromWorker(num: number): Promise<void>;
  copyVerse(key: VerseKey, text: string): Promise<boolean>;
  shareVerse(key: VerseKey, text: string): Promise<ShareResult>;
  /** FLOW-05 range variant: joined verse texts + the canonical `?ayah=` permalink. */
  shareRange(from: VerseKey, to: VerseKey, verses: readonly ShareVerse[]): Promise<ShareResult>;
}

export function createReader(): ReaderApi {
  const core = createReaderCore();
  const persistence = createReaderPersistence(core);
  const session = createReaderSession(core, persistence);
  const settings = createReaderSettings(core, persistence);
  const annotations = createAnnotations(core, persistence);
  const verseCache = createVerseCache(core);
  const share = createReaderShare();

  return {
    hydrate: (modeOverride?: ReaderMode) => persistence.hydrate(modeOverride),
    dispose: () => persistence.dispose(),

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

    get current() {
      return session.current;
    },

    get openNote() {
      return session.openNote;
    },
    toggleNote: (key: VerseKey) => session.toggleNote(key),

    setCurrent: (num: number) => session.setCurrent(num),
    openVerse: (num: number) => session.openVerse(num),
    markRead: (num: number, n: number, sourceId?: string) => session.markRead(num, n, sourceId),

    get arabicSizePx() {
      return settings.arabicSizePx;
    },
    bigger: () => settings.bigger(),
    smaller: () => settings.smaller(),
    get arabicFont() {
      return settings.arabicFont;
    },
    setArabicFont: (id: ArabicFontId) => settings.setArabicFont(id),
    get arabicScript() {
      return settings.arabicScript;
    },
    setArabicScript: (id: QuranSourceId) => settings.setArabicScript(id),
    get translationSizePx() {
      return settings.translationSizePx;
    },
    growTranslation: () => settings.growTranslation(),
    shrinkTranslation: () => settings.shrinkTranslation(),
    get translationFamily() {
      return settings.translationFamily;
    },
    setTranslationFamily: (family: TranslationFamily) => settings.setTranslationFamily(family),
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

    get position() {
      return session.position;
    },
    setPosition: (position: ReaderPositionState | null) => session.setPosition(position),

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
    get lastReadAnchor() {
      return annotations.lastReadAnchor;
    },
    setLastReadAnchor: (anchor: LastReadAnchor) => session.setLastReadAnchor(anchor),
    get pendingAnchor() {
      return session.pendingAnchor;
    },
    setPendingAnchor: (anchor: LastReadAnchor | null) => session.setPendingAnchor(anchor),
    consumePendingAnchor: () => session.consumePendingAnchor(),
    progressFor: (num: number) => annotations.progressFor(num),

    versesFor: (num: number) => verseCache.versesFor(num),
    seedAyahs: (ayahs) => verseCache.seedAyahs(ayahs),
    refreshFromWorker: (num: number) => verseCache.refreshFromWorker(num),

    copyVerse: (key: VerseKey, text: string) => share.copyVerse(key, text),
    shareVerse: (key: VerseKey, text: string) => share.shareVerse(key, text),
    shareRange: (from: VerseKey, to: VerseKey, verses: readonly ShareVerse[]) =>
      share.shareRange(from, to, verses),
  };
}

export const reader: ReaderApi = createReader();
