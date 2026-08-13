import type { QuranData } from "$lib/data/quran-data";

/**
 * Cross-checks a worker/API-fetched ayah's global index against the
 * catalogue's own numbering, so a corrupted or mismatched read never renders
 * silently. Shared by SurahReader and RangeReader, which both pass this to
 * `quranWorker.readRange`.
 */
export function ayahIndexValidator(
  quranData: QuranData,
): (globalIndex: number, surah: number, ayah: number) => boolean {
  return (globalIndex, surah, ayah) => quranData.globalIndexOf(surah, ayah) === globalIndex;
}
