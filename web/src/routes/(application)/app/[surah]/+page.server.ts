import { error } from "@sveltejs/kit";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readSurahRouteData } from "$lib/server/quran-surah-page";
import { requireSurah } from "$lib/server/reader-route-guards";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return QURAN_DATA.surahs.map((s) => ({ surah: s.slug }));
}

export const load: PageServerLoad = ({ params }) => {
  const surah = requireSurah(params.surah);
  const data = readSurahRouteData(surah, 1);
  if (!data) throw error(404, `Unknown surah: ${params.surah}`);
  return data;
};
