import { loadTranslationRangeData } from "$lib/server/quran-translation-page";
import { markTranslationPending, requireRangeIndex } from "$lib/server/reader-route-guards";

import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const index = requireRangeIndex("juz", params.n);
  const data = await loadTranslationRangeData("juz", index, params.lang, params.translator, fetch);
  markTranslationPending(setHeaders, data.ayahs);
  return data;
};
