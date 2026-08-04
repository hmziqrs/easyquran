import { error } from "@sveltejs/kit";
import { loadTranslationRangeData } from "$lib/server/quran-translation-page";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch }) => {
  const index = Number(params.n);
  if (!Number.isInteger(index) || index < 1 || index > 30) {
    throw error(404, `Unknown juz: ${params.n}`);
  }
  return loadTranslationRangeData("juz", index, params.lang, params.translator, fetch);
};
