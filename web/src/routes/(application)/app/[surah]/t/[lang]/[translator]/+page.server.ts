import { error } from "@sveltejs/kit";
import { QURAN_DATA } from "$lib/server/quran-data";
import { loadTranslationSurahRouteData } from "$lib/server/quran-translation-page";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch }) => {
  const surah = QURAN_DATA.surahBySlug(params.surah);
  if (!surah) throw error(404, `Unknown surah: ${params.surah}`);
  const data = await loadTranslationSurahRouteData(
    surah,
    1,
    params.lang,
    params.translator,
    fetch,
  );
  if (!data) throw error(404, `Unknown Surah page: 1`);
  return data;
};
