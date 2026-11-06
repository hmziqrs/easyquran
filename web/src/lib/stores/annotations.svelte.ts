import { parseVerseKey } from "#lib/bookmarks/schema.js";
import { peekQuranData } from "#lib/data/quran-data-client.js";
import type { VerseKey } from "#lib/data/quran.js";
import { sessions } from "#lib/sessions/store.svelte.js";

import type { LastReadAnchor, NoteMeta, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

/** One saved note with its verse coordinates, in mushaf order (the library's base list). */
export interface NoteEntry {
  readonly key: VerseKey;
  readonly surah: number;
  readonly ayah: number;
  readonly text: string;
  /** Null for notes written before timestamps existed. */
  readonly updatedAt: number | null;
  /** Translation the note was written in; null = Arabic reader. */
  readonly sourceId: string | null;
}

/** What a removed note held, so Undo can put it back exactly. */
export interface NoteSnapshot {
  readonly text: string;
  readonly meta: NoteMeta | null;
}

/** The thread the reading context comes from: this tab's, else the newest open one. */
function contextThread() {
  return sessions.current ?? sessions.latest;
}

export function createAnnotations(core: ReaderCore, persistence: ReaderPersistence) {
  // Writes one note's text and context; empty text removes the note entirely.
  function commit(key: VerseKey, text: string, meta: NoteMeta | null): void {
    if (text.length === 0) {
      delete core.s.notes[key];
      delete core.s.noteMeta[key];
    } else {
      core.s.notes[key] = text;
      core.s.noteMeta[key] = meta ?? { updatedAt: Date.now(), sourceId: null };
    }
    persistence.scheduleNoteWrite();
  }

  return {
    getNote(key: VerseKey): string {
      // Owner sync first (post-auth account vs `local`): reads must never surface
      // a previous owner's notes after a login/logout transition (EC-17).
      persistence.syncNotesOwner();
      return core.s.notes[key] ?? "";
    },
    /** Saves the note text. `sourceId` is the translation being read (null = Arabic). */
    setNote(key: VerseKey, v: string, sourceId: string | null = null): void {
      persistence.syncNotesOwner();
      commit(key, v, v.length === 0 ? null : { updatedAt: Date.now(), sourceId });
    },
    getNoteMeta(key: VerseKey): NoteMeta | null {
      persistence.syncNotesOwner();
      return core.s.noteMeta[key] ?? null;
    },
    /** Every note with text, in mushaf order. Reactive; the caller syncs the owner (`syncNotes`). */
    get noteEntries(): readonly NoteEntry[] {
      const entries: NoteEntry[] = [];
      for (const [key, text] of Object.entries(core.s.notes)) {
        const coords = parseVerseKey(key);
        if (coords === null || text.length === 0) continue;
        // SAFETY: parseVerseKey only accepts the `s:a` VerseKey shape, so key is a valid VerseKey.
        const verseKey = key as VerseKey;
        const meta: NoteMeta | undefined = core.s.noteMeta[verseKey];
        entries.push({
          key: verseKey,
          surah: coords.surah,
          ayah: coords.ayah,
          text,
          updatedAt: meta?.updatedAt ?? null,
          sourceId: meta?.sourceId ?? null,
        });
      }
      return entries.sort((a, b) => a.surah - b.surah || a.ayah - b.ayah);
    },
    /** Removes a note and returns what it held, for Undo. Null when there was no note. */
    removeNote(key: VerseKey): NoteSnapshot | null {
      persistence.syncNotesOwner();
      const text = core.s.notes[key];
      if (text === undefined) return null;
      const snapshot: NoteSnapshot = { text, meta: core.s.noteMeta[key] ?? null };
      commit(key, "", null);
      return snapshot;
    },
    /** Puts back a removed note with its original timestamp and source. */
    restoreNote(key: VerseKey, snapshot: NoteSnapshot): void {
      persistence.syncNotesOwner();
      commit(key, snapshot.text, snapshot.meta);
    },
    /** Re-resolves the notes owner after an auth change (EC-17). Call from an effect, not a derived. */
    syncNotes(): void {
      persistence.syncNotesOwner();
    },

    /** Where reading continues: this tab's thread, else the newest open thread. */
    get lastRead(): { num: number; n: number; sourceId?: string } | null {
      const thread = contextThread();
      if (thread === null) return null;
      return thread.sourceId !== null
        ? { num: thread.surah, n: thread.ayah, sourceId: thread.sourceId }
        : { num: thread.surah, n: thread.ayah };
    },
    get hasLastRead(): boolean {
      return contextThread() !== null;
    },
    get lastReadRef(): string {
      const thread = contextThread();
      if (thread === null) return "";
      const name = peekQuranData()?.surahByNum(thread.surah)?.name ?? `Surah ${thread.surah}`;
      return `${name} ${thread.surah}:${thread.ayah}`;
    },
    get lastReadAnchor(): LastReadAnchor | null {
      const thread = contextThread();
      if (thread === null || thread.localPage === null || thread.ratio === null) return null;
      return {
        verseKey: `${thread.surah}:${thread.ayah}`,
        localPage: thread.localPage,
        ratio: thread.ratio,
      };
    },
    progressFor(num: number): number | null {
      const furthest = sessions.furthestIn(num);
      if (furthest === null) return null;
      const total = peekQuranData()?.surahByNum(num)?.ayahCount ?? 0;
      return total > 0 ? Math.min(1, furthest / total) : null;
    },
  };
}
