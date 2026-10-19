import { browser } from "$app/env";

import type { ReaderPositionState } from "#lib/data/mushaf-divisions.js";
import type { VerseKey } from "#lib/data/quran.js";
import { sessions } from "#lib/sessions/store.svelte.js";

import type { BrowseMode, LastReadAnchor, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createReaderSession(core: ReaderCore, persistence: ReaderPersistence) {
  // Reading position lives on the tab's thread (#lib/sessions): position
  // writes go straight to the sessions store, never into the reader blob.
  return {
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

    get current(): number {
      return core.s.current;
    },

    get position(): ReaderPositionState | null {
      return core.s.position;
    },
    setPosition(position: ReaderPositionState | null): void {
      core.s.position = position;
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
      core.s.openNote = null;
      persistence.writeNow();
    },
    /** Jump to a verse (search hit, palette, resume): the reader route resolves the thread. */
    openVerse(num: number): void {
      core.s.current = num;
      core.nav.bump();
      core.s.browse = "surah";
      core.s.openNote = null;
      persistence.writeNow();
      if (browser) window.scrollTo(0, 0);
    },
    markRead(num: number, n: number, sourceId?: string): void {
      sessions.cursor(num, n, sourceId ?? null);
    },
    setLastReadAnchor(anchor: LastReadAnchor): void {
      sessions.anchor(anchor.localPage, anchor.ratio);
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
