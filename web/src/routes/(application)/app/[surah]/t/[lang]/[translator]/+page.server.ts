import { error } from "@sveltejs/kit";
import { loadTranslationSurahRouteData } from "$lib/server/quran-translation-page";
import { markTranslationPending, requireSurah } from "$lib/server/reader-route-guards";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const surah = requireSurah(params.surah);
  const data = await loadTranslationSurahRouteData(surah, 1, params.lang, params.translator, fetch);
  if (!data) throw error(404, `Unknown Surah page: 1`);
  markTranslationPending(setHeaders, data.pageData.ayahs);
  return data;
};
