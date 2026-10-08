import { loadTranslationRangeData } from "#lib/server/quran-translation-page.js";
import { markTranslationPending, requireRangeIndex } from "#lib/server/reader-route-guards.js";

import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const index = requireRangeIndex("hizb", params.n);
  const data = await loadTranslationRangeData("hizb", index, params.lang, params.translator, fetch);
  markTranslationPending(setHeaders, data.ayahs);
  return data;
};
