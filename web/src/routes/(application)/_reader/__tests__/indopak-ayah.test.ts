import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/state", () => ({ page: { url: { hash: "" }, params: {} } }));
vi.mock("$lib/stores/reader.svelte", () => ({
  reader: { isVerseMode: true, isReadingMode: false },
}));
vi.mock("../VerseTools.svelte", () => ({ default: () => {} }));

import { QuranScript } from "$lib/data/quran-types";
import { indopakEnding } from "$lib/quran/view/indopak";

import ReadingAyah from "../ReadingAyah.svelte";
import VerseRow from "../VerseRow.svelte";

const cases = [
  "يَعۡلَمُوۡنَ\uE01A\u200F",
  "الضَّآلِّيۡنَ\uE022",
  "مُؤۡمِنِيۡنَ\uE022ؕ",
  "عَلَيۡهِمۡ \uE021ۙ غَيۡرِ",
  "لِيَسُـوْۤء\uE004ا",
  "عَظِيۡمٌ \uE022",
  "بِسْمِ اللّٰهِ",
  "تَسۡرَحُوۡنَ\uE01B",
  "شِيۡبَا\uE01C  ۖ",
  "بِمَلُوۡمٍ\uE01C\uE01A",
  "الۡخَسِرِيۡنَ \uE01Aۙ",
  "رَّحِيۡمٌ ۙ",
  "سَرِيۡعُ الۡعِقَابِ\uE01A ۖ وَاِنَّهٗ لَـغَفُوۡرٌ رَّحِيۡمٌ\uE022",
];

describe("IndoPak end signs", () => {
  it("preserves codepoints and order while separating only the end annotation", () => {
    for (const text of cases) {
      const ending = indopakEnding(text);
      expect(ending.body + ending.lastWord + ending.sign + ending.following).toBe(text);
    }
    expect(indopakEnding(cases[0]!)).toMatchObject({
      lastWord: "يَعۡلَمُوۡنَ",
      sign: "\uE01A\u200F",
    });
    expect(indopakEnding(cases[2]!)).toMatchObject({ sign: "\uE022", following: "ؕ" });
    expect(indopakEnding("عَلَيۡهِمۡ \uE021ۙ غَيۡرِ").sign).toBe("");
    for (const text of cases) {
      const ending = indopakEnding(text);
      expect(ending.lastWord).toMatch(/\p{L}/u);
      expect(ending.annotations.map((annotation) => annotation.text).join("")).toBe(
        ending.sign + ending.following,
      );
      expect(ending.bodyParts.map((part) => part.text).join("")).toBe(ending.body);
    }
    expect(indopakEnding("بِمَلُوۡمٍ\uE01C\uE01A").annotations).toHaveLength(2);
    const paired = indopakEnding(cases.at(-1)!);
    const inline = paired.bodyParts.find((part) => part.annotations.length > 0);
    expect(inline?.text).toBe("\uE01A ۖ");
    expect(inline?.annotations.at(-1)).toMatchObject({ widthEm: 0.467, indentEm: 0.212 });
  });

  it("splits the body into Quran.com-style word boxes without changing text", () => {
    for (const text of cases) {
      const ending = indopakEnding(text);
      const words = ending.words
        .map((word) => word.parts.map((part) => part.text).join("") + word.gap)
        .join("");
      expect(words).toBe(ending.body);
      expect(ending.lastWordText + ending.lastWordSpace).toBe(ending.lastWord);
      for (const word of ending.words) expect(word.parts.length).toBeGreaterThan(0);
    }
    const paired = indopakEnding(cases.at(-1)!);
    const texts = paired.words.map((word) => word.parts.map((part) => part.text).join(""));
    const tokens = cases.at(-1)!.split(" ");
    expect(texts).toEqual([tokens[0], `${tokens[1]} ${tokens[2]}`, tokens[3], tokens[4]]);
    expect(paired.words.map((word) => word.stop)).toEqual([false, true, false, false]);
  });

  it("keeps standalone and ZWSP-joined pause signs with the preceding word", () => {
    const standalone = indopakEnding("السَّمَآءُ\u200B ؕ بَنٰهَا\uE01F");
    expect(standalone.words.map((word) => word.parts.map((part) => part.text).join(""))).toEqual([
      "السَّمَآءُ\u200B ؕ",
    ]);
    expect(standalone.words[0]?.stop).toBe(true);
    const joined = indopakEnding("لِاَنۡفُسِكُمۡ\u200B\uE01Eوَاِنۡ اَسَاۡتُمۡ فَلَهَا");
    expect(joined.words.map((word) => word.parts.map((part) => part.text).join(""))).toEqual([
      "لِاَنۡفُسِكُمۡ\u200B\uE01E",
      "وَاِنۡ",
      "اَسَاۡتُمۡ",
    ]);
    const final = indopakEnding("كُمۡ\u200B\uE01Eوَاِنۡ");
    expect(final.lastWord).toBe("وَاِنۡ");
    expect(final.body).toBe("كُمۡ\u200B\uE01E");
  });

  it("draws source whitespace inside end clusters at zero width without dropping it", () => {
    const ending = indopakEnding("الۡمُطۡمَئِنَّةُ\u2003\uE01C\u2003\u06D6");
    expect(ending.lastWordText).toBe("الۡمُطۡمَئِنَّةُ");
    expect(ending.lastWordSpace).toBe("\u2003");
    expect(ending.annotations.map(({ mark, space }) => [mark, space])).toEqual([
      ["\uE01C", "\u2003"],
      ["\u06D6", ""],
    ]);
  });

  for (const [name, component] of Object.entries({ ReadingAyah, VerseRow })) {
    it(`${name} renders original text once and keeps the mark with the number`, async () => {
      for (const text of cases) {
        const target = document.createElement("div");
        const instance = mount(component, {
          target,
          props: { text, n: 101, vKey: "2:101", script: QuranScript.IndoPak },
        });
        const verse = target.querySelector("[data-indopak-ayah]")!;
        const copy = verse.cloneNode(true);
        if (!(copy instanceof Element)) throw new Error("Missing IndoPak text element");
        copy.querySelector("[data-indopak-ornament]")!.remove();
        expect(copy.textContent).toBe(text);
        expect(verse.querySelectorAll(".ayah-ornament")).toHaveLength(1);
        const ornament = verse.querySelector(".indopak-final-word .ayah-ornament");
        expect(ornament?.textContent).toBe("\u06DD۱۰۱");
        expect(ornament?.getAttribute("lang")).toBe("ur");
        for (const word of verse.querySelectorAll(".indopak-word")) {
          expect(word.querySelector(".ayah-ornament")).toBeNull();
        }
        await unmount(instance);
      }
    });

    it(`${name} keeps Uthmani on its original rendering path`, async () => {
      const target = document.createElement("div");
      const instance = mount(component, {
        target,
        props: { text: "ٱللَّهُ", n: 1, vKey: "112:1", script: QuranScript.Uthmani },
      });
      expect(target.querySelector("[data-indopak-ayah]")).toBeNull();
      expect(target.querySelectorAll(".ayah-ornament")).toHaveLength(1);
      expect(target.querySelector(".verse-text")?.textContent).toContain("ٱللَّهُ");
      await unmount(instance);
    });
  }
});
