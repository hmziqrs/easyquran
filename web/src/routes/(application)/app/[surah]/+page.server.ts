import { error } from "@sveltejs/kit";
import { QURAN_DATA } from "$lib/server/quran-data";
import { readSurahRouteData } from "$lib/server/quran-surah-page";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return QURAN_DATA.surahs.map((s) => ({ surah: s.slug }));
}

export const load: PageServerLoad = ({ params }) => {
  const cat = QURAN_DATA.surahBySlug(params.surah);
  if (!cat) {
    throw error(404, `Unknown surah: ${params.surah}`);
  }
  return readSurahRouteData(cat, 1)!;
};
