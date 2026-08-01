/* ════════════════════════════════════════════════════════════════════════
   verse-cache.svelte.ts — the per-open-surah synchronous verse cache.

   A SvelteMap<number, string[]> (REACTIVE collection — the original plain Map
   silently failed to notify consumers; see docs/svelte-improvements.md §2.A).
   Seeded from prerendered page.data, refreshed best-effort from the sqlite-wasm
   Worker. The Worker refresh is guarded by the core's nav token AND a current-
   surah check so a response for a previously-open surah can never clobber the
   one now on screen.
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import type { ReaderCore } from "./reader-core.svelte";

export function createVerseCache(core: ReaderCore) {
  return {
    versesFor(num: number): string[] {
      return core.versesBySurah.get(num) ?? [];
    },
    seedSurah(num: number, verses: string[]): void {
      if (verses.length) core.versesBySurah.set(num, verses);
    },
    /**
     * Best-effort refresh of a surah's verses from the sqlite-wasm Worker,
     * guarded by the nav token so a response for a previously-open surah is
     * discarded. No-op until the Worker is ready; never throws (the prerendered
     * sync cache already serves the open surah, so failure is silently absorbed).
     */
    async refreshFromWorker(num: number): Promise<void> {
      if (!browser) return;
      const token = core.nav.token;
      try {
        const { quranWorker } = await import("$lib/quran/worker-client");
        if (!quranWorker.ready) return;
        const source = await quranWorker.readSurah(num);
        if (token !== core.nav.token) return;
        if (num !== core.s.current) return;
        if (source.verses.length) core.versesBySurah.set(num, source.verses);
      } catch {
      }
    },
  };
}
