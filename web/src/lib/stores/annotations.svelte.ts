import { peekQuranData } from "#lib/data/quran-data-client.js";
import type { VerseKey } from "#lib/data/quran.js";
import { sessions } from "#lib/sessions/store.svelte.js";

import type { LastReadAnchor, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

/** The thread the reading context comes from: this tab's, else the newest open one. */
function contextThread() {
  return sessions.current ?? sessions.latest;
}

export function createAnnotations(core: ReaderCore, persistence: ReaderPersistence) {
  return {
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
