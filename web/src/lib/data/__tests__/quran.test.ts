import {
  parseKey,
  surahAyahPath,
  surahLocalPagePath,
  surahPath,
  toArabicDigits,
  verseKey,
} from "$lib/data/quran";
import { RangeKind } from "$lib/data/quran-data";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readSurahLocalPageData } from "$lib/server/quran-surah-page";
import { describe, expect, it } from "vite-plus/test";

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

  it("tiles every Surah into 662 clipped Mushaf pages without crossing boundaries", () => {
    const pages = QURAN_DATA.surahs.flatMap((surah) => [...QURAN_DATA.surahLocalPages(surah.num)]);
    expect(pages).toHaveLength(662);

    for (const surah of QURAN_DATA.surahs) {
      const localPages = QURAN_DATA.surahLocalPages(surah.num);
      expect(localPages[0]).toMatchObject({ startAyah: 1, localPage: 1 });
      expect(localPages.at(-1)).toMatchObject({ endAyah: surah.ayahCount });
      for (let i = 1; i < localPages.length; i += 1) {
        expect(localPages[i]!.startAyah).toBe(localPages[i - 1]!.endAyah + 1);
      }
    }

    expect(QURAN_DATA.surahLocalPage(1, 1)).toMatchObject({
      globalPage: 1,
      first: "1:1",
      last: "1:7",
    });
    expect(QURAN_DATA.surahLocalPage(2, 1)).toMatchObject({
      globalPage: 2,
      startAyah: 1,
      endAyah: 5,
    });
    expect(QURAN_DATA.surahLocalPages(2).at(-1)).toMatchObject({ endAyah: 286 });
    expect(QURAN_DATA.surahLocalPageForAyah(2, 255)?.localPage).toBeGreaterThan(1);
  });
});

describe("routing and formatting helpers", () => {
  it("builds a Surah path from selected route metadata", () => {
    const fatihah = QURAN_DATA.surahByNum(1)!;
    expect(surahPath(fatihah)).toBe("/app/al-fatihah");
    expect(surahLocalPagePath(fatihah, 1)).toBe("/app/al-fatihah");
    expect(surahLocalPagePath(fatihah, 2)).toBe("/app/al-fatihah/page/2");
    expect(surahAyahPath(fatihah, 2, 5)).toBe("/app/al-fatihah/page/2#ayah-1-5");
  });

  it("converts western digits to Arabic-Indic", () => {
    expect(toArabicDigits(0)).toBe("٠");
    expect(toArabicDigits(123)).toBe("١٢٣");
  });
});

describe("Surah-local server payload", () => {
  it("reads only the requested Al-Baqarah page", () => {
    const baqarah = QURAN_DATA.surahByNum(2)!;
    const first = readSurahLocalPageData(baqarah, 1)!;
    expect(first.ayahs.map((ayah) => ayah.key)).toEqual(["2:1", "2:2", "2:3", "2:4", "2:5"]);
    expect(first.ayahs.some((ayah) => ayah.key === "2:286")).toBe(false);

    const last = readSurahLocalPageData(baqarah, first.pageCount)!;
    expect(last.page.endAyah).toBe(286);
    expect(last.ayahs.at(-1)?.key).toBe("2:286");
  });
});
