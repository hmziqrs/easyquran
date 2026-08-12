import { browser } from "$app/environment";
import type { VerseKey } from "$lib/data/quran";
import type { BrowseMode, LastReadAnchor, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createReaderSession(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    get query(): string {
      return core.s.query;
    },
    get hasQuery(): boolean {
      return core.s.query.trim().length > 0;
    },
    setQuery(v: string): void {
      core.s.query = v;
    },
    clearQuery(): void {
      core.s.query = "";
    },

    get browseMode(): BrowseMode {
      return core.s.browse;
    },
    get browseSurah(): boolean {
      return core.s.browse === "surah";
    },
    get browseAyah(): boolean {
      return core.s.browse === "ayah";
    },
    get browseJuz(): boolean {
      return core.s.browse === "juz";
    },
    get browsePage(): boolean {
      return core.s.browse === "page";
    },
    setBrowse(browse: BrowseMode): void {
      core.s.browse = browse;
    },

    get openNote(): VerseKey | null {
      return core.s.openNote;
    },
    toggleNote(key: VerseKey): void {
      const next = core.s.openNote === key ? null : key;
      core.s.openNote = next;
      if (next === null) persistence.flushNoteWrite();
    },

    setCurrent(num: number): void {
      core.s.current = num;
      core.nav.bump();
      core.s.query = "";
      core.s.openNote = null;
      persistence.writeNow();
    },
    openVerse(num: number, n: number, sourceId?: string): void {
      core.s.current = num;
      core.nav.bump();
      core.s.query = "";
      core.s.browse = "surah";
      core.s.openNote = null;
      core.s.lastRead = sourceId !== undefined ? { num, n, sourceId } : { num, n };
      core.s.lastReadAnchor = null;
      persistence.writeNow();
      if (browser) window.scrollTo(0, 0);
    },
    markRead(num: number, n: number, sourceId?: string): void {
      const current = core.s.lastRead;
      if (current?.num === num && current.n === n && current.sourceId === sourceId) return;
      core.s.lastRead = sourceId !== undefined ? { num, n, sourceId } : { num, n };
      persistence.writeNow();
    },
    clearReadingPosition(): void {
      core.s.lastRead = null;
      core.s.lastReadAnchor = null;
      core.s.pendingAnchor = null;
      persistence.writeNow();
    },
    setLastReadAnchor(anchor: LastReadAnchor | null): void {
      core.s.lastReadAnchor = anchor;
      persistence.scheduleAnchorWrite();
    },
    get pendingAnchor(): LastReadAnchor | null {
      return core.s.pendingAnchor;
    },
    setPendingAnchor(anchor: LastReadAnchor | null): void {
      core.s.pendingAnchor = anchor;
    },
    consumePendingAnchor(): LastReadAnchor | null {
      const anchor = core.s.pendingAnchor;
      core.s.pendingAnchor = null;
      return anchor;
    },
  };
}
