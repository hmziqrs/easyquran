import { dev } from "$app/environment";
import { createQuranData, RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";
import { error } from "@sveltejs/kit";

import quranDataRaw from "../../../../../static/quran-meta/quran-data.json";
import { HOME_VARIANTS, type SurahCard } from "../variants";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = ({ params }) => {
  if (!dev) error(404, "Not found");
  const preset = HOME_VARIANTS.find((candidate) => candidate.id === params.variant);
  if (!preset) error(404, "Unknown homepage variant");

  const quranData = createQuranData(quranDataRaw);
  const surahs: SurahCard[] = quranData.surahs.map((surah) => ({
    num: surah.num,
    slug: surah.slug,
    name: surah.name,
    meaning: surah.meaning,
    arabic: surah.arabic,
    ayahCount: surah.ayahCount,
    place: surah.place === "medinan" ? "Medinan" : "Meccan",
  }));

  return {
    variant: preset.id,
    surahs,
    surahCount: surahs.length,
    juzCount: quranData.rangeCount(RangeKind.Juz) || RANGE_COUNTS[RangeKind.Juz],
    pageCount: quranData.rangeCount(RangeKind.Page) || RANGE_COUNTS[RangeKind.Page],
  };
};
