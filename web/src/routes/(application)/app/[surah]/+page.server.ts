// SSG server load for /app/<slug>. Reads the verbatim Uthmani verse text from
// the checked-in quran-uthmani.sqlite through node:sqlite at `vite build`, so
// every one of the 114 prerendered pages ships real verse HTML for SEO and first
// paint — with no running backend. `entries()` enumerates the 114 catalog slugs.
//
// At runtime (after hydration) verses are also held in the reader's sync cache
// and served by the sqlite-wasm Worker / live API; this load only seeds first
// paint and the cache.
import { error } from "@sveltejs/kit";
import { surahBySlug } from "$lib/data/quran";
import { CATALOG } from "$lib/data/quran-meta";
import { readSurahVerses, validateUthmani } from "$lib/server/quran-sqlite";
import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return CATALOG.map((s) => ({ surah: s.slug }));
}

// Fail the build/dev fast if the Uthmani source drifted from its golden digest
// or row count (docs/quran-api.md §3.3, §4). Runs once per process load.
const source = validateUthmani();
if (!source.ok) {
  throw new Error(
    `[quran-sqlite] Uthmani source validation failed: rows=${source.rows} (want 6236), ` +
      `sha256=${source.sha256}. The DB and the golden constant in site.ts are out of sync.`,
  );
}

export const load: PageServerLoad = ({ params }) => {
  const cat = surahBySlug(params.surah);
  // Unknown slug → 404 rather than silently rendering Al-Fatihah.
  if (cat.slug !== params.surah) {
    throw error(404, `Unknown surah: ${params.surah}`);
  }
  return { surah: { ...cat, verses: readSurahVerses(cat.num) } };
};
