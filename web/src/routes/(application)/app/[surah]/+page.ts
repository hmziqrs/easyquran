// Prerender one static HTML page per available surah slug so deep links like
// /app/al-baqarah are real, SSR-correct files (indexable, no SPA fallback
// needed). The URL param — not the persisted reader store — drives which surah
// renders, so there is no hydration mismatch. When the full Tanzil set is
// wired in, expand SURAHS and these entries grow automatically.
import { SURAHS } from "$lib/data/quran";

export const prerender = true;

export function entries() {
  return SURAHS.map((s) => ({ surah: s.slug }));
}
