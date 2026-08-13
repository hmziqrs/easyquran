// Param guards every reader `+page.server.ts` repeats. Range bounds come from the baked
// `RANGE_COUNTS`, so no route hard-codes 604/30.
import { error, redirect } from "@sveltejs/kit";
import { RANGE_COUNTS, RangeKind } from "$lib/data/quran-data";
import type { CatalogEntry } from "$lib/data/quran-types";
import { QURAN_DATA } from "$lib/server/quran-data";

export function requireSurah(slug: string): CatalogEntry {
  const surah = QURAN_DATA.surahBySlug(slug);
  if (!surah) throw error(404, `Unknown surah: ${slug}`);
  return surah;
}

export function requireRangeIndex(kind: "juz" | "page", raw: string): number {
  const index = Number(raw);
  const max = RANGE_COUNTS[kind === "juz" ? RangeKind.Juz : RangeKind.Page];
  if (!Number.isInteger(index) || index < 1 || index > max) {
    throw error(404, `Unknown ${kind}: ${raw}`);
  }
  return index;
}

export function rangeEntries(kind: "juz" | "page"): { n: string }[] {
  const count = RANGE_COUNTS[kind === "juz" ? RangeKind.Juz : RangeKind.Page];
  return Array.from({ length: count }, (_, i) => ({ n: String(i + 1) }));
}

/**
 * Surah-local page 2..n. Page 1 is not addressable under `/page/1` — it 308s to the canonical
 * Surah root so both spellings never index separately.
 */
export function requireLocalPageBeyondFirst(raw: string, canonical: `/app/${string}`): number {
  const localPage = Number(raw);
  if (!Number.isSafeInteger(localPage) || localPage < 1) {
    throw error(404, `Unknown Surah page: ${raw}`);
  }
  if (localPage === 1) throw redirect(308, canonical);
  return localPage;
}

/**
 * A translated route renders with an empty ayah list when the upstream API is unreachable; the
 * shell is still useful, but it must never be indexed or cached as the real thing.
 */
export function markTranslationPending(
  setHeaders: (headers: Record<string, string>) => void,
  ayahs: readonly unknown[],
): void {
  if (ayahs.length > 0) return;
  setHeaders({ "x-eq-translation-pending": "1", "x-robots-tag": "noindex, follow" });
}
