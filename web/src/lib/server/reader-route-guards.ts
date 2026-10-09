// Param guards every reader `+page.server.ts` repeats. Range bounds come from the baked
// `RANGE_COUNTS` (plus the hizb derivation), so no route hard-codes 604/30/60.
import { error } from "@sveltejs/kit";

import { HIZB_COUNT } from "#lib/data/mushaf-divisions.js";
import { RANGE_COUNTS, RangeKind } from "#lib/data/quran-data.js";
import type { Ayah, CatalogEntry, RangeRouteKind } from "#lib/data/quran-types.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

export function requireSurah(slug: string): CatalogEntry {
  const surah = QURAN_DATA.surahBySlug(slug);
  if (!surah) throw error(404, `Unknown surah: ${slug}`);
  return surah;
}

const RANGE_COUNT_BY_KIND: Readonly<Record<RangeRouteKind, number>> = Object.freeze({
  juz: RANGE_COUNTS[RangeKind.Juz],
  page: RANGE_COUNTS[RangeKind.Page],
  hizb: HIZB_COUNT,
});

export function requireRangeIndex(kind: RangeRouteKind, raw: string): number {
  const index = Number(raw);
  const max = RANGE_COUNT_BY_KIND[kind];
  if (!Number.isInteger(index) || index < 1 || index > max) {
    throw error(404, `Unknown ${kind}: ${raw}`);
  }
  return index;
}

export function rangeEntries(kind: RangeRouteKind): { n: string }[] {
  return Array.from({ length: RANGE_COUNT_BY_KIND[kind] }, (_, i) => ({ n: String(i + 1) }));
}

/**
 * A translated route renders with an empty ayah list when the upstream API is unreachable; the
 * shell is still useful, but it must never be indexed or cached as the real thing.
 */
export function markTranslationPending(
  setHeaders: (headers: Record<string, string>) => void,
  ayahs: readonly Ayah[],
): void {
  if (ayahs.length > 0) return;
  setHeaders({ "x-eq-translation-pending": "1", "x-robots-tag": "noindex, follow" });
}
