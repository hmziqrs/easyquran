import type { VerseKey } from "$lib/data/quran";
import { peekQuranData } from "$lib/data/quran-data-client";
import type { LastReadAnchor, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createAnnotations(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    isBookmarked(key: VerseKey): boolean {
      return !!core.s.bookmarks[key];
    },
    toggleBookmark(key: VerseKey): void {
      if (core.s.bookmarks[key]) delete core.s.bookmarks[key];
      else core.s.bookmarks[key] = true;
      persistence.writeNow();
    },

    getNote(key: VerseKey): string {
      return core.s.notes[key] ?? "";
    },
    setNote(key: VerseKey, v: string): void {
      core.s.notes[key] = v;
      persistence.scheduleNoteWrite();
    },

    get lastRead(): { num: number; n: number; sourceId?: string } | null {
      return core.s.lastRead;
    },
    get hasLastRead(): boolean {
      return core.s.lastRead !== null;
    },
    get lastReadRef(): string {
      const lr = core.s.lastRead;
      if (!lr) return "";
      const name = peekQuranData()?.surahByNum(lr.num)?.name ?? `Surah ${lr.num}`;
      return `${name} ${lr.num}:${lr.n}`;
    },
    get lastReadAnchor(): LastReadAnchor | null {
      return core.s.lastReadAnchor;
    },
  };
}
