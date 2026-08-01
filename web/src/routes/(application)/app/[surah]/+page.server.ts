import { error } from "@sveltejs/kit";
import { surahBySlug } from "$lib/data/quran";
import { CATALOG } from "$lib/data/quran-meta";
import { readSurahText, validateReaderSource } from "$lib/server/quran-sqlite";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return CATALOG.map((s) => ({ surah: s.slug }));
}

const source = validateReaderSource();
if (!source.ok) {
  throw new Error(
    `[quran-sqlite] Uthmani source validation failed: rows=${source.rows} (want 6236), ` +
      `sha256=${source.sha256}. The DB and the golden constant in site.ts are out of sync.`,
  );
}

export const load: PageServerLoad = ({ params }) => {
  const cat = surahBySlug(params.surah);
  if (cat.slug !== params.surah) {
    throw error(404, `Unknown surah: ${params.surah}`);
  }
  return { surah: { ...cat, ...readSurahText(cat.num) } };
};
