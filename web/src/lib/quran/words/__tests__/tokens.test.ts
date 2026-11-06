import { describe, expect, it } from "vite-plus/test";

import { QuranScript } from "#lib/data/quran-types.js";
import { sliceWords } from "#lib/quran/words/tokens.js";
import { wordKey } from "#lib/quran/words/types.js";

// Reader text samples, one per script in use. Only the bytes matter here:
// slicing must never rewrite them, whatever the script's markup.
const SAMPLES: ReadonlyArray<readonly [string, string]> = [
  [QuranScript.Uthmani, "بِسْمِ ٱللَّهِ ٱلرَّحْمَٰنِ ٱلرَّحِيمِ"],
  [QuranScript.SimpleClean, "بسم الله الرحمن الرحيم"],
  [QuranScript.IndoPak, "بِسۡمِ ٱللَّهِ ٱلرَّحۡمَٰنِ ٱلرَّحِيمِ"],
  [QuranScript.Tajweed, "[h:1468[ٱ]لْ[n[ٰ]ٱ ‌[s[ـَ]"],
  ["spacing", "  ٱلْحَمْدُ لِلَّهِ\t\nرَبِّ  "],
];

describe("sliceWords byte fidelity", () => {
  it.each(SAMPLES)("joining slices reproduces the %s reader bytes", (_script, text) => {
    const slices = sliceWords(text);
    expect(slices.map((slice) => slice.text).join("")).toBe(text);
    for (const slice of slices) {
      expect(text.slice(slice.start, slice.end)).toBe(slice.text);
    }
  });

  it("returns no slices for empty text", () => {
    expect(sliceWords("")).toEqual([]);
  });
});

describe("sliceWords word coordinates", () => {
  it("numbers words 1-based and leaves whitespace unnumbered", () => {
    const words = sliceWords("  بسم الله\tالرحمن ").filter((slice) => slice.word !== null);
    expect(words.map((slice) => slice.word)).toEqual([1, 2, 3]);
    expect(words.map((slice) => slice.text)).toEqual(["بسم", "الله", "الرحمن"]);
  });

  it("keeps Arabic marks and joiners inside their word", () => {
    const words = sliceWords("بِسْمِ ٱللَّهِ ‌ٱلرَّحِيمِ").filter((slice) => slice.word !== null);
    expect(words).toHaveLength(3);
    // ZWNJ (U+200C) stays in the word: text is verbatim from the source.
    expect(words[2]?.text).toBe("\u200cٱلرَّحِيمِ");
  });

  it("maps a word to a canonical surah:ayah:word key", () => {
    expect(wordKey({ surah: 1, ayah: 1, word: 2 })).toBe("1:1:2");
  });
});
