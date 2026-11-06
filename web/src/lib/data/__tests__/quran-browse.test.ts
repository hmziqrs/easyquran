import { describe, expect, it } from "vite-plus/test";

import { SurahSort, surahsInRevelationOrder, surahsInSortOrder } from "#lib/data/quran-browse.js";
import { RangeKind } from "#lib/data/quran-data.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

describe("revelation order", () => {
  const revealed = surahsInRevelationOrder(QURAN_DATA.surahs);

  it("is a permutation of all 114 surahs", () => {
    expect(revealed).toHaveLength(114);
    expect(new Set(revealed.map((s) => s.num)).size).toBe(114);
  });

  it("puts the first revealed surah first and never goes backwards", () => {
    expect(revealed[0]).toMatchObject({ num: 96, revelationOrder: 1 });
    for (let i = 1; i < revealed.length; i += 1) {
      expect(revealed[i - 1]!.revelationOrder).toBeLessThanOrEqual(revealed[i]!.revelationOrder);
    }
  });

  it("breaks revelation ties in mushaf order", () => {
    for (let i = 1; i < revealed.length; i += 1) {
      const prev = revealed[i - 1]!;
      const next = revealed[i]!;
      if (prev.revelationOrder === next.revelationOrder) expect(prev.num).toBeLessThan(next.num);
    }
  });

  it("leaves the mushaf sort as the input order and does not mutate the input", () => {
    const before = QURAN_DATA.surahs.map((s) => s.num);
    expect(surahsInSortOrder(QURAN_DATA.surahs, SurahSort.Mushaf)).toBe(QURAN_DATA.surahs);
    expect(surahsInSortOrder(QURAN_DATA.surahs, SurahSort.Revelation)).toEqual(revealed);
    expect(QURAN_DATA.surahs.map((s) => s.num)).toEqual(before);
  });
});

describe("ruku, manzil and sajda browse data", () => {
  it("covers the whole Quran with ruku ranges in order", () => {
    const rukus = QURAN_DATA.ranges(RangeKind.Ruku);
    expect(rukus).toHaveLength(556);
    expect(rukus[0]?.startGlobal).toBe(1);
    expect(rukus.at(-1)?.endGlobal).toBe(6236);
    for (let i = 1; i < rukus.length; i += 1) {
      expect(rukus[i]!.startGlobal).toBe(rukus[i - 1]!.endGlobal + 1);
    }
  });

  it("has seven manzils that tile the Quran", () => {
    const manzils = QURAN_DATA.ranges(RangeKind.Manzil);
    expect(manzils.map((m) => m.index)).toEqual([1, 2, 3, 4, 5, 6, 7]);
    expect(manzils[0]?.startGlobal).toBe(1);
    expect(manzils.at(-1)?.endGlobal).toBe(6236);
  });

  it("lists the fifteen sajdas, each resolvable to its verse", () => {
    const sajdas = QURAN_DATA.sajdas();
    expect(sajdas).toHaveLength(15);
    for (const sajda of sajdas) {
      expect(QURAN_DATA.sajdaAt(sajda.surah, sajda.ayah)).toEqual(sajda);
      expect(QURAN_DATA.globalIndexOf(sajda.surah, sajda.ayah)).toBe(sajda.globalIndex);
    }
  });
});
