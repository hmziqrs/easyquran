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
        expect(verse.querySelector(".indopak-final-word .ayah-ornament")?.textContent).toBe(
          "\u06DD١٠١",
        );
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
