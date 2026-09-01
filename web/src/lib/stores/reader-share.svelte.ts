import { browser } from "$app/environment";
import { parseKey, type VerseKey } from "$lib/data/quran";
import { peekQuranData } from "$lib/data/quran-data-client";
import { stripTajweedMarkup } from "$lib/quran/view/tajweed";

function verseRef(key: VerseKey): string {
  const { num, n } = parseKey(key);
  const name = peekQuranData()?.surahByNum(num)?.name ?? `Surah ${num}`;
  return `${name} ${num}:${n}`;
}

function verseShareText(key: VerseKey, text: string): string {
  // Clipboard/share are plain-text views: tajweed markup renders as colors in
  // the reader, but must not leak `[h:1[ٱ]` tokens into copied text.
  return `${stripTajweedMarkup(text)}\n${verseRef(key)}`;
}

export function createReaderShare() {
  return {
    async copyVerse(key: VerseKey, text: string): Promise<boolean> {
      if (!browser) return false;
      try {
        await navigator.clipboard.writeText(verseShareText(key, text));
        return true;
      } catch {
        return false;
      }
    },
    async shareVerse(key: VerseKey, verse: string): Promise<"shared" | "copied" | "failed"> {
      if (!browser) return "failed";
      const ref = verseRef(key);
      const text = verseShareText(key, verse);
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
