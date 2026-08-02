import { browser } from "$app/environment";
import { parseKey, verseKey, type VerseKey } from "$lib/data/quran";
import type { ReaderCore } from "./reader-core.svelte";

export function createVerseCache(core: ReaderCore) {
  return {
    versesFor(num: number): string[] {
      const verses: string[] = [];
      for (const [key, text] of core.verseTextByKey) {
        const coordinate = parseKey(key);
        if (coordinate.num === num) verses[coordinate.n - 1] = text;
      }
      return verses;
    },
    seedAyahs(ayahs: readonly { key: VerseKey; text: string }[]): void {
      for (const ayah of ayahs) core.verseTextByKey.set(ayah.key, ayah.text);
    },
    async refreshFromWorker(num: number): Promise<void> {
      if (!browser) return;
      const token = core.nav.token;
      try {
        const { quranWorker } = await import("$lib/quran/worker-client");
        await quranWorker.whenReady();
        const source = await quranWorker.readSurah(num);
        if (token !== core.nav.token) return;
        if (num !== core.s.current) return;
        source.verses.forEach((text, index) => {
          core.verseTextByKey.set(verseKey(num, index + 1), text);
        });
      } catch {}
    },
  };
}
