import { QURAN_DATA_URL } from "#lib/data/quran-data-client.js";
import { createQuranData } from "#lib/data/quran-data.js";

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

export const load: PageLoad = async ({ fetch, data }) => {
  const response = await fetch(QURAN_DATA_URL, {
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
  // Scheme A: /surah also carries the server discovery load (+page.server.ts
  // → readerPrerenderHrefs). A universal load REPLACES the page data unless
  // it spreads the server result through, so the ReaderPrerenderLinks anchors
  // must ride along here.
  return { ...data, surahs };
};
