import { QuranScript } from "$lib/data/quran-types";
import {
  isTajweedScript,
  parseTajweedSegments,
  stripTajweedMarkup,
  tajweedRuleColor,
} from "$lib/quran/view/tajweed";
import { describe, expect, it } from "vite-plus/test";

// Real bytes from db/quran/arabic/quran-tajweed.sqlite (1:1, 2:255 excerpt).
const FATIHAH_1 = "بِسْمِ [h:1[ٱ]للَّهِ [h:2[ٱ][l[ل]رَّحْمَ[n[ـٰ]نِ [h:3[ٱ][l[ل]رَّح[p[ِي]مِ";
const AYAT_AL_KURSI_EXCERPT = "ٱللَّهُ ل[o[َآ] إِلَ[n[ـٰ]هَ إِلَّا هُوَ [h:1468[ٱ]لْحَىُّ";
// Real bytes from db/quran/arabic/quran-tajweed.sqlite (4:158, 77:20): rule
// letter `b` = idgham mutaqaribayn (lam assimilating into ra, qaf into kaf).
// \u200c is the ZWNJ Tanzil puts before the small-waqf sign, kept verbatim.
const NISA_4_158 = "بَ[b:3279[ل] رَّفَعَهُ [h:3280[ٱ]للَّهُ إِلَيْهِ\u200cۚ وَكَانَ [h:2653[ٱ]للَّهُ عَزِيزًا حَكِيمًا";
const MURSALAT_77_20 = "أَلَمْ نَخْلُ[b:14217[ق]كّ[w:14218[ُم م]ّ[a:840[ِن م]ّ[o[َا]ٓ[a:10629[ءٍ م]َّه[p[ِي]نٍ";

describe("parseTajweedSegments", () => {
  it("passes non-tajweed text through as a single plain run", () => {
    expect(parseTajweedSegments("بِسْمِ ٱللَّهِ")).toEqual([{ text: "بِسْمِ ٱللَّهِ", rule: null }]);
  });

  it("parses id and bare rule forms with surrounding plain runs", () => {
    const segments = parseTajweedSegments("ٱللَّهُ ل[o[َآ] إِلَ[n[ـٰ]هَ");
    expect(segments).toEqual([
      { text: "ٱللَّهُ ل", rule: null },
      { text: "َآ", rule: "o" },
      { text: " إِلَ", rule: null },
      { text: "ـٰ", rule: "n" },
      { text: "هَ", rule: null },
    ]);
  });

  it("parses a full fatihah 1:1 sample without losing or inventing text", () => {
    const segments = parseTajweedSegments(FATIHAH_1);
    expect(segments.filter((s) => s.rule !== null).length).toBeGreaterThan(0);
    expect(segments.reduce((acc, s) => acc + s.text, "")).toBe(stripTajweedMarkup(FATIHAH_1));
  });

  it("treats literal brackets that do not form markup as plain text", () => {
    expect(parseTajweedSegments("[2] footnote [x[not a rule]")).toEqual([
      { text: "[2] footnote [x[not a rule]", rule: null },
    ]);
  });

  it("recovers when a segment is never closed (no text loss)", () => {
    expect(parseTajweedSegments("آمن [g[قلب")).toEqual([
      { text: "آمن ", rule: null },
      { text: "قلب", rule: "g" },
    ]);
  });

  it("rejects id forms without digits or opening bracket", () => {
    expect(parseTajweedSegments("[h:x[ٱ] [n[")).toEqual([{ text: "[h:x[ٱ] [n[", rule: null }]);
  });

  it("parses rule letter b (idgham mutaqaribayn) from real DB rows", () => {
    expect(parseTajweedSegments(NISA_4_158)).toEqual([
      { text: "بَ", rule: null },
      { text: "ل", rule: "b" },
      { text: " رَّفَعَهُ ", rule: null },
      { text: "ٱ", rule: "h" },
      { text: "للَّهُ إِلَيْهِ\u200cۚ وَكَانَ ", rule: null },
      { text: "ٱ", rule: "h" },
      { text: "للَّهُ عَزِيزًا حَكِيمًا", rule: null },
    ]);
    expect(parseTajweedSegments(MURSALAT_77_20).slice(0, 2)).toEqual([
      { text: "أَلَمْ نَخْلُ", rule: null },
      { text: "ق", rule: "b" },
    ]);
  });
});

describe("stripTajweedMarkup", () => {
  it("removes markup, keeps every colored run", () => {
    expect(stripTajweedMarkup(FATIHAH_1)).toBe("بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ");
  });

  it("is identity for plain text", () => {
    expect(stripTajweedMarkup("[2] tanzil footnote marker")).toBe("[2] tanzil footnote marker");
  });

  it("round-trips real ayat al-kursi bytes", () => {
    expect(stripTajweedMarkup(AYAT_AL_KURSI_EXCERPT)).toBe("ٱللَّهُ لَآ إِلَـٰهَ إِلَّا هُوَ ٱلْحَىُّ");
  });

  it("strips rule b runs like every other rule (4:158)", () => {
    expect(stripTajweedMarkup(NISA_4_158)).toBe("بَل رَّفَعَهُ ٱللَّهُ إِلَيْهِ\u200cۚ وَكَانَ ٱللَّهُ عَزِيزًا حَكِيمًا");
  });
});

describe("tajweedRuleColor + isTajweedScript", () => {
  it("maps every known rule letter to a color", () => {
    const letters = [
      "h",
      "s",
      "l",
      "n",
      "p",
      "m",
      "q",
      "o",
      "c",
      "f",
      "w",
      "i",
      "a",
      "u",
      "d",
      "b",
      "g",
    ] as const;
    for (const letter of letters) expect(tajweedRuleColor(letter)).toMatch(/^#[0-9a-f]{6}$/u);
  });

  it("classifies only the tajweed script", () => {
    expect(isTajweedScript(QuranScript.Tajweed)).toBe(true);
    expect(isTajweedScript(QuranScript.Uthmani)).toBe(false);
    expect(isTajweedScript(QuranScript.IndoPak)).toBe(false);
    expect(isTajweedScript(QuranScript.SimpleClean)).toBe(false);
    expect(isTajweedScript(QuranScript.Translation)).toBe(false);
  });
});
