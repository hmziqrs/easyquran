import { error } from "@sveltejs/kit";
import { translationSurahPath } from "$lib/data/quran";
import { loadTranslationSurahRouteData } from "$lib/server/quran-translation-page";
import {
  markTranslationPending,
  requireLocalPageBeyondFirst,
  requireSurah,
} from "$lib/server/reader-route-guards";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const surah = requireSurah(params.surah);
  const localPage = requireLocalPageBeyondFirst(
    params.localPage,
    translationSurahPath(surah.slug, params.lang, params.translator, 1),
  );
  const data = await loadTranslationSurahRouteData(
    surah,
    localPage,
    params.lang,
    params.translator,
    fetch,
  );
  if (!data) throw error(404, `Unknown Surah page: ${params.localPage}`);
  markTranslationPending(setHeaders, data.pageData.ayahs);
  return data;
};
