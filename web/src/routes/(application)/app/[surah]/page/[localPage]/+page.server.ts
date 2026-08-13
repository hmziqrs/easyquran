import { surahLocalPagePath } from "$lib/data/quran";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readSurahRouteData } from "$lib/server/quran-surah-page";
import { requireLocalPageBeyondFirst, requireSurah } from "$lib/server/reader-route-guards";
import { error } from "@sveltejs/kit";

import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return QURAN_DATA.surahs.flatMap((surah) =>
    Array.from({ length: QURAN_DATA.surahLocalPageCount(surah.num) - 1 }, (_, index) => ({
      surah: surah.slug,
      localPage: String(index + 2),
    })),
  );
}

export const load: PageServerLoad = ({ params }) => {
  const surah = requireSurah(params.surah);
  const localPage = requireLocalPageBeyondFirst(params.localPage, surahLocalPagePath(surah, 1));
  const data = readSurahRouteData(surah, localPage);
  if (!data) throw error(404, `Unknown Surah page: ${params.localPage}`);
  return data;
};
