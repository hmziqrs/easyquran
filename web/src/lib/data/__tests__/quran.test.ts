import { describe, expect, it } from "vite-plus/test";
import { parseKey, surahPath, toArabicDigits, verseKey } from "$lib/data/quran";
import { RangeKind } from "$lib/data/quran-data";
import { QURAN_DATA } from "$lib/server/quran-data";

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
});

describe("immutable Quran metadata", () => {
  it("retains every canonical catalog field and coordinate", () => {
    expect(QURAN_DATA.surahs).toHaveLength(114);
    expect(QURAN_DATA.coordinates.rowCount).toBe(6236);
    expect(QURAN_DATA.surahByNum(1)).toMatchObject({
      slug: "al-fatihah",
      transliteration: "Al-Faatiha",
      meaning: "The Opening",
      startGlobal: 1,
    });
    expect(QURAN_DATA.verseKeyAtGlobal(262)).toBe("2:255");
    expect(QURAN_DATA.globalIndexOf(2, 255)).toBe(262);
  });

  it("fails explicitly instead of silently falling back to Al-Fatihah", () => {
    expect(QURAN_DATA.surahByNum(0)).toBeUndefined();
    expect(QURAN_DATA.surahBySlug("does-not-exist")).toBeUndefined();
    expect(QURAN_DATA.verseKeyAtGlobal(0)).toBeUndefined();
    expect(QURAN_DATA.globalIndexOf(2, 287)).toBeUndefined();
  });

  it("exposes each range family through lazy accessors", () => {
    expect(QURAN_DATA.rangeCount(RangeKind.Page)).toBe(604);
    expect(QURAN_DATA.rangeCount(RangeKind.Juz)).toBe(30);
    expect(QURAN_DATA.rangeCount(RangeKind.Ruku)).toBe(556);
    expect(QURAN_DATA.rangeCount(RangeKind.HizbQuarter)).toBe(240);
    expect(QURAN_DATA.rangeCount(RangeKind.Manzil)).toBe(7);
    expect(QURAN_DATA.rangeByIndex(RangeKind.Page, 1)).toMatchObject({
      first: "1:1",
      last: "1:7",
    });
    expect(QURAN_DATA.rangeByIndex(RangeKind.Page, 605)).toBeUndefined();
    expect(QURAN_DATA.sajdas()).toHaveLength(15);
  });
});

describe("routing and formatting helpers", () => {
  it("builds a Surah path from selected route metadata", () => {
    const fatihah = QURAN_DATA.surahByNum(1)!;
    expect(surahPath(fatihah)).toBe("/app/al-fatihah");
    expect(surahPath(fatihah, 5)).toBe("/app/al-fatihah?verse=5");
  });

  it("converts western digits to Arabic-Indic", () => {
    expect(toArabicDigits(0)).toBe("٠");
    expect(toArabicDigits(123)).toBe("١٢٣");
  });
});
