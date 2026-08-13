import { QURAN_DATA, toSurahRenderMetadata } from "$lib/server/quran-data";
import { readSurahVerses } from "$lib/server/quran-sqlite";
import { error } from "@sveltejs/kit";

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
  const cat = QURAN_DATA.surahByNum(SPECIMEN_SURAH)!;
  return {
    variant: params.variant,
    surah: { ...toSurahRenderMetadata(cat), verses: readSurahVerses(cat.num) },
  };
};
