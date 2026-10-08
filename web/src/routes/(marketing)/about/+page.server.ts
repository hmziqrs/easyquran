import { TRANSLATION_CATALOGUE } from "#lib/quran/catalogue.js";
import type { AboutCounts } from "#lib/i18n/about-copy.js";

/**
 * Prerendered: the totals are counted from the baked catalogue at build time, so the copy
 * never states a stale number and the catalogue never ships to the about page's client.
 */
export const load = () => ({
  counts: {
    translations: TRANSLATION_CATALOGUE.length,
    languages: new Set(TRANSLATION_CATALOGUE.map((t) => t.language)).size,
  } satisfies AboutCounts,
});
