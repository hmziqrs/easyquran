import { error } from "@sveltejs/kit";
import { surahByNum } from "$lib/data/quran";
import { readSurahText } from "$lib/server/quran-sqlite";
import { READER_VARIANTS, isVariantId } from "../../_variants/registry";
import type { PageServerLoad } from "./$types";

export const prerender = true;

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
    surah: { ...cat, ...readSurahText(cat.num) },
  };
};
