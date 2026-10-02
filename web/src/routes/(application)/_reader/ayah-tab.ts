import type { QuranData } from "$lib/data/quran-data";
import type { CatalogEntry } from "$lib/data/quran-types";

/**
 * Which surah the sidebar Ayah tab shows — from ANY reader route (D7): the
 * route's surah slug wins; otherwise the reader's current/last-read surah
 * (the field `reader.setCurrent` maintains); otherwise Al-Fatihah.
 */
export function ayahTabSurah(
  quranData: QuranData,
  params: Record<string, string | undefined>,
  readerCurrent: number,
): CatalogEntry {
  const slug = params.surah;
  if (slug) {
    const fromRoute = quranData.surahBySlug(slug);
    if (fromRoute) return fromRoute;
  }
  return quranData.surahByNum(readerCurrent) ?? quranData.surahs[0]!;
}
