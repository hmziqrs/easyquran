/* ════════════════════════════════════════════════════════════════════════
   reader-session.svelte.ts — transient reading-session state.

   Cohesive responsibility: the sidebar search query, the browse list mode
   (Surah/Ayah/Juz/Page), which verse's note panel is open, and navigation
   (setCurrent/openVerse) which bumps the nav token and persists immediately
   (navigation is a discrete action, not a keystroke stream).

   `openNote`/`toggleNote` live here because they are UI state (which panel is
   open); note *data* and its debounced persistence live in annotations. Closing
   a note panel flushes any pending debounced save so the user's last edit lands
   before they navigate away.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { VerseKey } from "$lib/data/quran";
import type { BrowseMode, ReaderCore } from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createReaderSession(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    // ── search ───────────────────────────────────────────────────────
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

    // ── sidebar browse mode ──────────────────────────────────────────
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

    // ── open note panel (UI state) ───────────────────────────────────
    get openNote(): VerseKey | null {
      return core.s.openNote;
    },
    toggleNote(key: VerseKey): void {
      const next = core.s.openNote === key ? null : key;
      core.s.openNote = next;
      // Closing the panel saves any in-flight edit immediately.
      if (next === null) persistence.flushNoteWrite();
    },

    // ── navigation ───────────────────────────────────────────────────
    /** Change the current surah (URL-driven). Resets query + open note and
     *  invalidates any in-flight Worker refresh for the prior surah. */
    setCurrent(num: number): void {
      core.s.current = num;
      core.nav.bump();
      core.s.query = "";
      core.s.openNote = null;
      persistence.writeNow();
    },
    /** Jump to a specific verse (search result / bookmark / continue-reading). */
    openVerse(num: number, n: number): void {
      core.s.current = num;
      core.nav.bump();
      core.s.query = "";
      core.s.browse = "surah";
      core.s.openNote = null;
      core.s.lastRead = { num, n };
      persistence.writeNow();
      if (browser) window.scrollTo(0, 0);
    },
  };
}
