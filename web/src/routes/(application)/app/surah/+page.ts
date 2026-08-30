import { asset } from "$app/paths";
import { createQuranData } from "$lib/data/quran-data";

import type { PageLoad } from "./$types";

export const prerender = true;

export interface SurahIndexRow {
  num: number;
  slug: string;
  name: string;
  arabic: string;
  transliteration: string;
  meaning: string;
  place: "meccan" | "medinan";
  ayahCount: number;
}

export const load: PageLoad = async ({ fetch }) => {
  const response = await fetch(asset("/quran-meta/quran-data.json"), {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`[surah index] quran-data fetch failed: ${response.status}`);
  }
  const quran = createQuranData(await response.json());
  const surahs: SurahIndexRow[] = quran.surahs.map((surah) => ({
    num: surah.num,
    slug: surah.slug,
    name: surah.name,
    arabic: surah.arabic,
    transliteration: surah.transliteration,
    meaning: surah.meaning,
    place: surah.place,
    ayahCount: surah.ayahCount,
  }));
  return { surahs };
};
