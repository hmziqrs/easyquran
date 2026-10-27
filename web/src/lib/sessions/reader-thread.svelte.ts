import { tick, untrack } from "svelte";

import type { ReadingThread } from "./schema";
import { sessions, type ReaderTarget } from "./store.svelte";

/**
 * Resolve the tab's reading thread whenever a reader route shows something
 * new (`key` changes). When an existing thread was picked up with its cursor
 * elsewhere, `onResume` moves the reader there. Call from a reader component's
 * script (registers a client-only effect).
 */
export function trackReaderThread(opts: {
  readonly key: () => string;
  readonly target: () => ReaderTarget | null;
  readonly onResume: (thread: ReadingThread) => void;
  /** Fires with the resolved thread (entered, kept or resumed) each key change. */
  readonly onEnter?: (thread: ReadingThread) => void;
}): void {
  $effect(() => {
    opts.key();
    untrack(() => {
      const target = opts.target();
      if (target === null) return;
      const result = sessions.enter(target);
      opts.onEnter?.(result.thread);
      if (result.resumed) opts.onResume(result.thread);
    });
  });
}

/** Scroll a verse marker already on the page into view; false when it is not mounted. */
export async function revealVerseMarker(surah: number, ayah: number): Promise<boolean> {
  await tick();
  const marker = document.querySelector<HTMLElement>(`[data-verse-anchor="${surah}:${ayah}"]`);
  if (marker === null) return false;
  marker.scrollIntoView({ behavior: "auto", block: "center" });
  return true;
}
