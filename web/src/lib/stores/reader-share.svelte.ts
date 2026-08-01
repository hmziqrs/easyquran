/* ════════════════════════════════════════════════════════════════════════
   reader-share.svelte.ts — copy/share side effects + verse-share text.

   Pure text formatting (`verseShareText`) is separated from the clipboard /
   Web Share API calls so the format is testable without a browser. The
   side-effect helpers read from the shared verse cache (no Worker round-trip)
   and are no-ops on the server. Kept under stores/ (the doc's suggested
   quran/share-text.ts + quran/web-share.ts live outside this task's owned
   files; the split here still isolates the responsibility).
   ════════════════════════════════════════════════════════════════════════ */

import { browser } from "$app/environment";
import { parseKey, surahByNum, type VerseKey } from "$lib/data/quran";
import type { ReaderCore } from "./reader-core.svelte";

/** "Surah Name num:n" reference label, e.g. "Al-Baqarah 2:255". */
export function verseRef(key: VerseKey): string {
  const { num, n } = parseKey(key);
  return `${surahByNum(num).name} ${num}:${n}`;
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
