/* ════════════════════════════════════════════════════════════════════════
   annotations.svelte.ts — bookmarks, notes, and last-read data.

   These are the user's durable annotations over the text. Bookmark toggles are
   discrete clicks → immediate persistence. Notes change on every keystroke →
   debounced persistence (the trailing write is scheduled by the persistence
   facet and flushed on note-close / page-hide).

   Deeply reactive records are mutated DIRECTLY (no cloning). Svelte 5's `$state`
   proxy traps property add/delete, so `delete core.s.bookmarks[key]` and
   `core.s.notes[key] = v` notify consumers without an object spread. The
   removed clone was never the performance problem — debouncing the write was.
   ════════════════════════════════════════════════════════════════════════ */

import { surahByNum } from "$lib/data/quran";
import type { VerseKey } from "$lib/data/quran";
import type { ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createAnnotations(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    // ── bookmarks ────────────────────────────────────────────────────
    isBookmarked(key: VerseKey): boolean {
      return !!core.s.bookmarks[key];
    },
    toggleBookmark(key: VerseKey): void {
      if (core.s.bookmarks[key]) delete core.s.bookmarks[key];
      else core.s.bookmarks[key] = true;
      persistence.writeNow();
    },

    // ── notes (content + debounced persistence) ──────────────────────
    getNote(key: VerseKey): string {
      return core.s.notes[key] ?? "";
    },
    setNote(key: VerseKey, v: string): void {
      core.s.notes[key] = v;
      persistence.scheduleNoteWrite();
    },

    // ── last read ────────────────────────────────────────────────────
    get lastRead(): { num: number; n: number } | null {
      return core.s.lastRead;
    },
    get hasLastRead(): boolean {
      return core.s.lastRead !== null;
    },
    get lastReadRef(): string {
      const lr = core.s.lastRead;
      if (!lr) return "";
      return `${surahByNum(lr.num).name} ${lr.num}:${lr.n}`;
    },
  };
}
