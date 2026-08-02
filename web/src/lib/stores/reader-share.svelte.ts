import { browser } from "$app/environment";
import { parseKey, type VerseKey } from "$lib/data/quran";
import { peekQuranData } from "$lib/data/quran-data-client";
import type { ReaderCore } from "./reader-core.svelte";

export function verseRef(key: VerseKey): string {
  const { num, n } = parseKey(key);
  const name = peekQuranData()?.surahByNum(num)?.name ?? `Surah ${num}`;
  return `${name} ${num}:${n}`;
}

export function verseShareText(key: VerseKey, text: string): string {
  return `${text}\n${verseRef(key)}`;
}

export function createReaderShare(core: ReaderCore) {
  const verseText = (key: VerseKey): string => {
    const { num, n } = parseKey(key);
    return (core.versesBySurah.get(num) ?? [])[n - 1] ?? "";
  };

  return {
    async copyVerse(key: VerseKey): Promise<boolean> {
      if (!browser) return false;
      try {
        await navigator.clipboard.writeText(verseShareText(key, verseText(key)));
        return true;
      } catch {
        return false;
      }
    },
    async shareVerse(key: VerseKey): Promise<"shared" | "copied" | "failed"> {
      if (!browser) return "failed";
      const ref = verseRef(key);
      const text = verseShareText(key, verseText(key));
      try {
        if (navigator.share) {
          await navigator.share({ title: ref, text });
          return "shared";
        }
        await navigator.clipboard.writeText(text);
        return "copied";
      } catch {
        return "failed";
      }
    },
  };
}
