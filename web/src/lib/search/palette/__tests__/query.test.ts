import { describe, expect, it } from "vite-plus/test";

import {
  arabicTermsFor,
  hasKeyword,
  isBareNumber,
  parseQuery,
  referenceNumbers,
  stripTrailingRef,
  termsFor,
} from "../query";
import { scoreArabic, scoreFields, scoreText } from "../scoring";

const ALIASES = ["surah", "sura", "s"] as const;

describe("parseQuery", () => {
  it("collapses whitespace and reports the leading word token", () => {
    const parsed = parseQuery("  Surah   Al   Kahf  ");
    expect(parsed.text).toBe("Surah Al Kahf");
    expect(parsed.keyword).toBe("surah");
    expect(parsed.afterKeyword).toBe("Al Kahf");
    expect(parsed.isEmpty).toBe(false);
  });

  it("reports no keyword when the query starts with a digit", () => {
    expect(parseQuery("2:255").keyword).toBeNull();
    expect(parseQuery("100").keyword).toBeNull();
  });

  it("extracts both parts of a surah:ayah reference across separators", () => {
    for (const raw of ["2:255", "2.255", "2 255", "2-255"]) {
      expect(parseQuery(raw).numbers, raw).toEqual([2, 255]);
    }
  });

  it("extracts a single trailing number after a keyword or a name", () => {
    expect(parseQuery("juz 5").numbers).toEqual([5]);
    expect(parseQuery("baqarah 255").numbers).toEqual([255]);
    expect(parseQuery("p100").numbers).toEqual([100]);
  });

  it("reports no numbers for purely textual queries", () => {
    expect(parseQuery("kahf").numbers).toEqual([]);
    expect(parseQuery("").numbers).toEqual([]);
    expect(parseQuery("").isEmpty).toBe(true);
  });

  it("normalizes Arabic for matching", () => {
    expect(parseQuery("ٱلْبَقَرَة").arabic).toBe(parseQuery("البقره").arabic);
  });

  it("recognizes only its own keyword aliases", () => {
    expect(hasKeyword(parseQuery("sura 18"), ALIASES)).toBe(true);
    expect(hasKeyword(parseQuery("juz 18"), ALIASES)).toBe(false);
    expect(hasKeyword(parseQuery("18"), ALIASES)).toBe(false);
  });

  it("strips a claimed keyword but keeps an unclaimed first word as search text", () => {
    expect(termsFor(parseQuery("sura kahf"), ALIASES)).toBe("kahf");
    expect(termsFor(parseQuery("kahf"), ALIASES)).toBe("kahf");
    expect(arabicTermsFor(parseQuery("sura البقرة"), ALIASES)).toBe(parseQuery("البقرة").arabic);
  });

  it("strips a trailing reference so a name can be matched alone", () => {
    expect(stripTrailingRef("baqarah 255")).toBe("baqarah");
    expect(stripTrailingRef("al-kahf")).toBe("al-kahf");
    expect(stripTrailingRef("255")).toBe("");
  });

  it("names the reference parts and flags a bare number", () => {
    expect(referenceNumbers(parseQuery("2:255"))).toEqual({ primary: 2, secondary: 255 });
    expect(referenceNumbers(parseQuery("juz 5"))).toEqual({ primary: 5 });
    expect(referenceNumbers(parseQuery("kahf"))).toBeNull();
    expect(isBareNumber(parseQuery("30"))).toBe(true);
    expect(isBareNumber(parseQuery("juz 30"))).toBe(false);
  });
});

describe("scoring", () => {
  it("ranks exact above prefix above word-start above substring above subsequence", () => {
    const exact = scoreText("Al-Kahf", "al-kahf");
    const prefix = scoreText("Al-Kahf", "al-k");
    const wordStart = scoreText("The Cave", "cave");
    const substring = scoreText("Al-Baqarah", "aqar");
    const subsequence = scoreText("Al-Baqarah", "alqh");
    expect(exact).toBeGreaterThan(prefix);
    expect(prefix).toBeGreaterThan(wordStart);
    expect(wordStart).toBeGreaterThan(substring);
    expect(substring).toBeGreaterThan(subsequence);
    expect(subsequence).toBeGreaterThan(0);
  });

  it("returns 0 for no match and for empty input", () => {
    expect(scoreText("Al-Kahf", "zzz")).toBe(0);
    expect(scoreText("Al-Kahf", "")).toBe(0);
    expect(scoreText("", "kahf")).toBe(0);
  });

  it("takes the best score across fields", () => {
    expect(scoreFields(["Al-Baqarah", undefined, "The Cow"], "the cow")).toBe(1);
  });

  it("matches Arabic through the shared normalization", () => {
    const needle = parseQuery("البقره").arabic;
    expect(scoreArabic("ٱلْبَقَرَة", needle)).toBeGreaterThan(0);
    expect(scoreArabic("الكهف", needle)).toBe(0);
    expect(scoreArabic("البقرة", "")).toBe(0);
  });
});
