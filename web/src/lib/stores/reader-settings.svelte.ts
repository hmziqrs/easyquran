/* ════════════════════════════════════════════════════════════════════════
   reader-settings.svelte.ts — reader-specific display settings.

   Arabic font size and reading mode are durable, discrete-click settings, so
   they persist immediately on change (no debouncing). Arabic font size is
   intentionally NOT merged into the global appearance store — it is a
   reader-specific preference and stays independently hydratable here.
   ════════════════════════════════════════════════════════════════════════ */

import {
  ARABIC_FONT_MAX,
  ARABIC_FONT_MIN,
  ARABIC_FONT_STEP,
  type ReaderCore,
  type ReaderMode,
} from "./reader-core.svelte";
import type { ReaderPersistence } from "./reader-persistence.svelte";

export function createReaderSettings(core: ReaderCore, persistence: ReaderPersistence) {
  return {
    // ── Arabic font size ─────────────────────────────────────────────
    get arabicSizePx(): string {
      return `${core.s.fontSize}px`;
    },
    bigger(): void {
      core.s.fontSize = Math.min(ARABIC_FONT_MAX, core.s.fontSize + ARABIC_FONT_STEP);
      persistence.writeNow();
    },
    smaller(): void {
      core.s.fontSize = Math.max(ARABIC_FONT_MIN, core.s.fontSize - ARABIC_FONT_STEP);
      persistence.writeNow();
    },

    // ── reading mode ─────────────────────────────────────────────────
    get mode(): ReaderMode {
      return core.s.mode;
    },
    get isVerseMode(): boolean {
      return core.s.mode === "verse";
    },
    get isReadingMode(): boolean {
      return core.s.mode === "reading";
    },
    setMode(mode: ReaderMode): void {
      if (core.s.mode === mode) return;
      core.s.mode = mode;
      persistence.writeNow();
    },
  };
}
