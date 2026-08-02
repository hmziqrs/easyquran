import { error, redirect } from "@sveltejs/kit";
import { surahLocalPagePath } from "$lib/data/quran";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readSurahRouteData } from "$lib/server/quran-surah-page";
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
  const surah = QURAN_DATA.surahBySlug(params.surah);
  if (!surah) throw error(404, `Unknown surah: ${params.surah}`);
  const localPage = Number(params.localPage);
  if (!Number.isSafeInteger(localPage) || localPage < 1) {
    throw error(404, `Unknown Surah page: ${params.localPage}`);
  }
  if (localPage === 1) throw redirect(308, surahLocalPagePath(surah, 1));
  const data = readSurahRouteData(surah, localPage);
  if (!data) throw error(404, `Unknown Surah page: ${params.localPage}`);
  return data;
};
