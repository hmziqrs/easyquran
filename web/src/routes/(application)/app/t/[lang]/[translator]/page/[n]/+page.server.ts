import { error } from "@sveltejs/kit";
import { translationIdFromSegments } from "$lib/data/quran";
import { findCatalogueEntry, resolveSourceCatalogue } from "$lib/quran/catalogue";
import { loadTranslationRangeData } from "$lib/server/quran-translation-page";
import type { PageServerLoad } from "./$types";

export const prerender = false;

export const load: PageServerLoad = async ({ params, fetch }) => {
  const index = Number(params.n);
  if (!Number.isInteger(index) || index < 1 || index > 604) {
    throw error(404, `Unknown page: ${params.n}`);
  }
  const sourceId = translationIdFromSegments(params.lang, params.translator);
  const catalogue = await resolveSourceCatalogue();
  if (catalogue.length > 0 && !findCatalogueEntry(catalogue, sourceId)) {
    throw error(404, `Unknown translation: ${params.lang}.${params.translator}`);
  }
  return loadTranslationRangeData("page", index, params.lang, params.translator, fetch);
};
