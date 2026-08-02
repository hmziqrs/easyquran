import { error } from "@sveltejs/kit";
import { QURAN_DATA, toSurahLink, toSurahRenderMetadata } from "$lib/server/quran-data";
import { readSurahText, validateReaderSource } from "$lib/server/quran-sqlite";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return QURAN_DATA.surahs.map((s) => ({ surah: s.slug }));
}

const source = validateReaderSource();
if (!source.ok) {
  throw new Error(
    `[quran-sqlite] Uthmani source validation failed: rows=${source.rows} (want 6236), ` +
      `sha256=${source.sha256}. The DB and the golden constant in site.ts are out of sync.`,
  );
}

export const load: PageServerLoad = ({ params }) => {
  const cat = QURAN_DATA.surahBySlug(params.surah);
  if (!cat) {
    throw error(404, `Unknown surah: ${params.surah}`);
  }
  return {
    surah: { ...toSurahRenderMetadata(cat), ...readSurahText(cat.num) },
    previous: cat.num > 1 ? toSurahLink(QURAN_DATA.surahByNum(cat.num - 1)!) : undefined,
    next: cat.num < 114 ? toSurahLink(QURAN_DATA.surahByNum(cat.num + 1)!) : undefined,
  };
};
