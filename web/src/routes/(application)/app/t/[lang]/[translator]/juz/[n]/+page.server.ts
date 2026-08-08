import { error } from "@sveltejs/kit";
import { loadTranslationRangeData } from "$lib/server/quran-translation-page";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch, setHeaders }) => {
  const index = Number(params.n);
  if (!Number.isInteger(index) || index < 1 || index > 30) {
    throw error(404, `Unknown juz: ${params.n}`);
  }
  const data = await loadTranslationRangeData("juz", index, params.lang, params.translator, fetch);
  if (data.ayahs.length === 0) {
    setHeaders({ "x-eq-translation-pending": "1", "x-robots-tag": "noindex, follow" });
  }
  return data;
};
