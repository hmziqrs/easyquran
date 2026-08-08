import { browser } from "$app/environment";
import { parseKey, type VerseKey } from "$lib/data/quran";
import type { ReaderCore } from "./reader-core.svelte";

export function createVerseCache(core: ReaderCore) {
  return {
    versesFor(num: number): string[] {
      void core.verseVersionBySurah.get(num);
      const sub = core.verseTextBySurah.get(num);
      if (!sub) return [];
      const verses: string[] = [];
      for (const [n, text] of sub) verses[n - 1] = text;
      return verses;
    },
    seedAyahs(ayahs: readonly { key: VerseKey; text: string }[]): void {
      const dirty = new Set<number>();
      for (const ayah of ayahs) {
        const { num, n } = parseKey(ayah.key);
        let sub = core.verseTextBySurah.get(num);
        if (!sub) {
          sub = new Map<number, string>();
          core.verseTextBySurah.set(num, sub);
        }
        sub.set(n, ayah.text);
        dirty.add(num);
      }
      for (const num of dirty) {
        core.verseVersionBySurah.set(num, (core.verseVersionBySurah.get(num) ?? 0) + 1);
      }
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
        let sub = core.verseTextBySurah.get(num);
        if (!sub) {
          sub = new Map<number, string>();
          core.verseTextBySurah.set(num, sub);
        }
        source.verses.forEach((text, index) => sub!.set(index + 1, text));
        core.verseVersionBySurah.set(num, (core.verseVersionBySurah.get(num) ?? 0) + 1);
      } catch {}
    },
  };
}
