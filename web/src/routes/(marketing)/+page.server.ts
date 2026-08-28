import { createQuranData, RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";

import quranDataRaw from "../../../static/quran-meta/quran-data.json";

export interface SurahCard {
  num: number;
  slug: string;
  name: string;
  meaning: string;
  arabic: string;
  ayahCount: number;
  place: string;
}

/**
 * The landing page is prerendered, so the full 114-surah index is baked into
 * the HTML at build time — no client fetch of the metadata bundle just to draw
 * the browse grid. The metric strip's numbers come from the same catalogue,
 * never hard-coded.
 */
export const load = () => {
  const data = createQuranData(quranDataRaw);
  const surahs = data.surahs.map((s) => ({
    num: s.num,
    slug: s.slug,
    name: s.name,
    meaning: s.meaning,
    arabic: s.arabic,
    ayahCount: s.ayahCount,
    place: s.place === "medinan" ? "Medinan" : "Meccan",
  }));
  return {
    surahs,
    surahCount: surahs.length,
    juzCount: data.rangeCount(RangeKind.Juz) || RANGE_COUNTS[RangeKind.Juz],
    pageCount: data.rangeCount(RangeKind.Page) || RANGE_COUNTS[RangeKind.Page],
  } satisfies {
    surahs: SurahCard[];
    surahCount: number;
    juzCount: number;
    pageCount: number;
  };
};
