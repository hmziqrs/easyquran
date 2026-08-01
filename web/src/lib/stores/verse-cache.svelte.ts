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
