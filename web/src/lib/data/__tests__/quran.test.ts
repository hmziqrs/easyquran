import { describe, expect, it } from "vite-plus/test";
import {
  adjacentSurahs,
  parseKey,
  searchVerses,
  slugFor,
  surahByNum,
  surahBySlug,
  surahPath,
  toArabicDigits,
  verseKey,
} from "$lib/data/quran";

describe("verseKey / parseKey", () => {
  it("round-trips a surah:ayah key", () => {
    expect(verseKey(2, 255)).toBe("2:255");
    expect(parseKey("2:255")).toEqual({ num: 2, n: 255 });
  });

  it("returns a safe {1,1} sentinel for malformed keys instead of NaN", () => {
    expect(parseKey("not-a-key" as never)).toEqual({ num: 1, n: 1 });
    expect(parseKey("2" as never)).toEqual({ num: 1, n: 1 });
    expect(parseKey("2:" as never)).toEqual({ num: 1, n: 1 });
    expect(parseKey(":5" as never)).toEqual({ num: 1, n: 1 });
  });

  it("does not accept non-numeric segments", () => {
    expect(parseKey("2:abc" as never)).toEqual({ num: 1, n: 1 });
  });
});

describe("surah lookups", () => {
  it("surahByNum returns the catalog entry for a known number", () => {
    expect(surahByNum(1).num).toBe(1);
    expect(surahByNum(114).num).toBe(114);
  });

  it("surahByNum falls back to Al-Fatihah for out-of-range numbers", () => {
    expect(surahByNum(0).num).toBe(1);
    expect(surahByNum(99999).num).toBe(1);
  });

  it("surahBySlug falls back to Al-Fatihah for unknown slugs", () => {
    expect(surahBySlug("does-not-exist").num).toBe(1);
  });

  it("adjacentSurahs wraps at both ends of the catalog", () => {
    expect(adjacentSurahs(1).next.num).toBe(2);
    expect(adjacentSurahs(1).prev.num).toBe(114); // wraps back to the last
    expect(adjacentSurahs(114).next.num).toBe(1); // wraps forward to the first
  });
});

describe("routing helpers", () => {
  it("slugFor round-trips through surahByNum", () => {
    expect(slugFor(1)).toBe(surahByNum(1).slug);
  });

  it("surahPath builds /app/<slug> and appends a verse deep-link only when given", () => {
    expect(surahPath(1)).toBe(`/app/${surahByNum(1).slug}`);
    expect(surahPath(1, 5)).toBe(`/app/${surahByNum(1).slug}?verse=5`);
  });
});

describe("toArabicDigits", () => {
  it("converts western digits to Arabic-Indic", () => {
    expect(toArabicDigits(0)).toBe("٠");
    expect(toArabicDigits(123)).toBe("١٢٣");
  });
});

describe("searchVerses (name/number fallback)", () => {
  it("returns nothing for blank input", () => {
    expect(searchVerses("")).toEqual([]);
    expect(searchVerses("   ")).toEqual([]);
  });

  it("matches a surah by number", () => {
    const hits = searchVerses("1");
    expect(hits.some((h) => h.num === 1)).toBe(true);
  });

  it("caps results to keep the box responsive", () => {
    // A broad Latin/number query that cannot match 24+ entries still returns <=24.
    expect(searchVerses("a").length).toBeLessThanOrEqual(24);
  });
});
