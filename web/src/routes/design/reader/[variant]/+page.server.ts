import { error } from "@sveltejs/kit";
import { QURAN_CATALOG, toSurahRenderMetadata } from "$lib/server/quran-metadata";
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
  const cat = QURAN_CATALOG.surahByNum(DEMO_SURAH)!;
  return {
    variant: params.variant,
    surah: { ...toSurahRenderMetadata(cat), ...readSurahText(cat.num) },
  };
};
