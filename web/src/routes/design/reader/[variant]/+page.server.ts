// SSG load for the reader variants. All three render the same surah from the
// same source the shipping reader uses, so the comparison is purely about
// layout — and so no Quranic text is ever hand-typed into the gallery.
import { error } from "@sveltejs/kit";
import { surahByNum } from "$lib/data/quran";
import { readSurahVerses } from "$lib/server/quran-sqlite";
import { READER_VARIANTS, isVariantId } from "../../_variants/registry";
import type { PageServerLoad } from "./$types";

export const prerender = true;

/** Al-Mulk (67) — 30 ayahs of middling length: long enough to show how a
 *  layout behaves when you scroll, short enough to take in at a glance. */
const DEMO_SURAH = 67;

export function entries() {
  return READER_VARIANTS.map((v) => ({ variant: v.id }));
}

export const load: PageServerLoad = ({ params }) => {
  if (!isVariantId(params.variant)) {
    throw error(404, `Unknown reader variant: ${params.variant}`);
  }
  const cat = surahByNum(DEMO_SURAH);
  return {
    variant: params.variant,
    surah: { ...cat, verses: readSurahVerses(cat.num) },
  };
};
