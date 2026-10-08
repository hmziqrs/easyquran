import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/state", () => ({ page: { url: { hash: "" }, params: {} } }));
vi.mock("#lib/stores/reader.svelte.js", () => ({
  reader: { isVerseMode: true, isReadingMode: false },
}));
vi.mock("../VerseTools.svelte", () => ({ default: () => {} }));

import { QuranScript } from "#lib/data/quran-types.js";
import { indopakEnding } from "#lib/quran/view/indopak.js";

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

  it.each(["\u06D9\uE01C", "\u06DA\uE01A", "\uE021\u06D9", "\uE01E\u0614"])(
    "isolates every mark in the inline sign pair %s",
    (pair) => {
      const text = `قَالَ${pair} ثُمَّ قَالَ`;
      const ending = indopakEnding(text);
      const cluster = ending.words[0]?.parts.find((part) => part.annotations.length > 0);
      expect(cluster?.annotations.map((item) => item.mark)).toEqual(Array.from(pair));
      expect(ending.bodyParts.map((part) => part.text).join("") + ending.lastWord).toBe(text);
    },
  );

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

  const [qala, araayta, idh] = [
    "\u0642\u064E\u0627\u0644\u064E",
    "\u0627\u064E\u0631\u064E\u0621\u064E\u064A\u06E1\u062A\u064E",
    "\u0627\u0650\u0630\u06E1",
  ];
  const glued: [source: string, text: string, words: string[], last: string][] = [
    [
      "ZWSP split without a sign",
      `${qala}\u200B${araayta} ${idh}`,
      [`${qala}\u200B`, araayta],
      idh,
    ],
    ["2:229 pause sign", `${qala} \u0615${araayta} ${idh}`, [`${qala} \u0615`, araayta], idh],
    [
      "2:282 ZWSP-led sign",
      `${qala} \u200B\u06DA${araayta} ${idh}`,
      [`${qala} \u200B\u06DA`, araayta],
      idh,
    ],
    ["4:9 private pause", `${qala} \uE01B${araayta} ${idh}`, [`${qala} \uE01B`, araayta], idh],
    [
      "19:17 cluster",
      `${qala} \uE01B\uE01E${araayta} ${idh}`,
      [`${qala} \uE01B\uE01E`, araayta],
      idh,
    ],
    [
      "91:14 spaced cluster",
      `${qala}\uE021 \uE01B\u2003 \u06D9${araayta} ${idh}`,
      [`${qala}\uE021 \uE01B\u2003 \u06D9`, araayta],
      idh,
    ],
    ["7:158 mark in the body", `${qala} \u06E8${araayta} ${idh}`, [`${qala} \u06E8`, araayta], idh],
    ["15:61 mark before the final word", `${qala} \u06E8${araayta}`, [`${qala} \u06E8`], araayta],
    ["18:63 pause before the final word", `${qala} \uE01C${araayta}`, [`${qala} \uE01C`], araayta],
    ["76:17 ink-free first box", `\u200B \u200B ${qala} ${idh}`, ["\u200B \u200B", qala], idh],
  ];

  it("starts every word box at a letter and keeps glued signs with the preceding word", () => {
    for (const [source, text, words, last] of glued) {
      const ending = indopakEnding(text);
      const boxes = ending.words.map((word) => word.parts.map((part) => part.text).join(""));
      expect(boxes, source).toEqual(words);
      expect(ending.lastWordText, source).toBe(last);
      expect(ending.body + ending.lastWord + ending.sign + ending.following, source).toBe(text);
      for (const box of boxes.slice(1)) expect(box, source).toMatch(/^\p{L}/u);
    }
    const stops = (text: string) => indopakEnding(text).words.map((word) => word.stop);
    expect(stops(`${qala} \u0615${araayta} ${idh}`)).toEqual([true, false]);
    expect(stops(`${qala} \uE01B\uE01E${araayta} ${idh}`)).toEqual([true, false]);
    expect(stops(`${qala} \u06E8${araayta} ${idh}`)).toEqual([false, false]);
  });

  it("draws whitespace inside the final word and inside sign marks without dropping it", () => {
    const final = indopakEnding(`${qala} ${araayta} \u200B\u06DA`);
    expect(final.lastWordText).toBe(`${araayta} \u200B\u06DA`);
    expect(final.lastWordSpace).toBe("");
    const inline = indopakEnding(`${qala}\u200B\uE01E \u200B\u06DA ${araayta} ${idh}`);
    const cluster = inline.words[0]?.parts.find((part) => part.annotations.length > 0);
    expect(cluster?.text).toBe("\uE01E \u200B\u06DA");
    expect(cluster?.annotations.map(({ mark, space }) => [mark, space])).toEqual([
      ["\uE01E", " \u200B"],
      ["\u06DA", ""],
    ]);
    expect(
      indopakEnding("\u0628\u0650\u0645\u064E\u0644\u064F\u0648\u06E1\u0645\u064D\uE01A\u200F")
        .annotations[0],
    ).toMatchObject({
      mark: "\uE01A\u200F",
      space: "",
    });
  });

  it("renders glued-sign verses with exact text and letter-led word boxes", async () => {
    for (const [source, text] of glued) {
      const target = document.createElement("div");
      const instance = mount(ReadingAyah, {
        target,
        props: { text, n: 7, vKey: "2:7", script: QuranScript.IndoPak },
      });
      const verse = target.querySelector("[data-indopak-ayah]")!;
      const copy = verse.cloneNode(true);
      if (!(copy instanceof Element)) throw new Error("Missing IndoPak text element");
      copy.querySelector("[data-indopak-ornament]")!.remove();
      expect(copy.textContent, source).toBe(text);
      const words = [...verse.querySelectorAll(".indopak-word")].slice(1);
      for (const word of words) expect(word.textContent, source).toMatch(/^\p{L}/u);
      await unmount(instance);
    }
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
