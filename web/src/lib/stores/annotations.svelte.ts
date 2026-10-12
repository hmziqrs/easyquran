import { peekQuranData } from "#lib/data/quran-data-client.js";
import type { VerseKey } from "#lib/data/quran.js";

import type { LastReadAnchor, ReaderCore, RecentsEntry } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createAnnotations(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    isBookmarked(key: VerseKey): boolean {
      return !!core.s.bookmarks[key];
    },
    get bookmarkedKeys(): VerseKey[] {
      return Object.keys(core.s.bookmarks);
    },
    /** v5 save stamps (epoch ms) beside `bookmarkedKeys`; missing entries are legacy (EC-46). */
    get bookmarkSavedAt(): Readonly<Record<VerseKey, number>> {
      return core.s.bookmarkSavedAt;
    },
    toggleBookmark(key: VerseKey): void {
      if (core.s.bookmarks[key]) {
        delete core.s.bookmarks[key];
        delete core.s.bookmarkSavedAt[key];
      } else {
        core.s.bookmarks[key] = true;
        // v5: anonymous bookmarks carry save timestamps; entries without one are
        // legacy (v≤4) and sort last downstream (EC-46).
        core.s.bookmarkSavedAt[key] = Date.now();
      }
      persistence.writeNow();
    },

    getNote(key: VerseKey): string {
      // Owner sync first (post-auth account vs `local`): reads must never surface
      // a previous owner's notes after a login/logout transition (EC-17).
      persistence.syncNotesOwner();
      return core.s.notes[key] ?? "";
    },
    setNote(key: VerseKey, v: string): void {
      persistence.syncNotesOwner();
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
    get recentReads(): readonly RecentsEntry[] {
      return core.s.recents;
    },
    progressFor(num: number): number | null {
      const entry = core.s.progress[num];
      if (!entry) return null;
      const total = peekQuranData()?.surahByNum(num)?.ayahCount ?? 0;
      return total > 0 ? Math.min(1, entry.furthestAyah / total) : null;
    },
  };
}
