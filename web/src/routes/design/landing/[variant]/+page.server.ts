import { error } from "@sveltejs/kit";
import { surahByNum } from "$lib/data/quran";
import { readSurahVerses } from "$lib/server/quran-sqlite";
import { LANDING_VARIANTS, isVariantId } from "../../_variants/registry";
import type { PageServerLoad } from "./$types";

export const prerender = true;

const SPECIMEN_SURAH = 1;

export function entries() {
  return LANDING_VARIANTS.map((v) => ({ variant: v.id }));
}

export const load: PageServerLoad = ({ params }) => {
  if (!isVariantId(params.variant)) {
    throw error(404, `Unknown landing variant: ${params.variant}`);
  }
  const cat = surahByNum(SPECIMEN_SURAH);
  return {
    variant: params.variant,
    surah: { ...cat, verses: readSurahVerses(cat.num) },
  };
};
