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
