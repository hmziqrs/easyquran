import type { CanonicalQuranCoordinates } from "$lib/data/quran-types";
import type { QuranCoordinateRow } from "$lib/quran/sql";

export function isCanonicalAyahCoordinate(
  globalIndex: number,
  surah: number,
  ayah: number,
  expected: CanonicalQuranCoordinates,
): boolean {
  const coordinates = expected.surahs[surah - 1];
  return (
    coordinates?.surah === surah &&
    ayah >= 1 &&
    ayah <= coordinates.ayahCount &&
    globalIndex === coordinates.startGlobal + ayah - 1
  );
}

export function validateCanonicalCoordinates(
  sourceProfile: string,
  rows: readonly QuranCoordinateRow[],
  expected: CanonicalQuranCoordinates,
): void {
  if (expected.surahs.length !== 114) {
    throw new Error(
      `[quran-source:${sourceProfile}] canonical surah count ${expected.surahs.length} != 114`,
    );
  }
  if (rows.length !== expected.rowCount) {
    throw new Error(
      `[quran-source:${sourceProfile}] coordinate row count ${rows.length} != ${expected.rowCount}`,
    );
  }

  let rowIndex = 0;
  for (const [surahIndex, surah] of expected.surahs.entries()) {
    const expectedSurah = surahIndex + 1;
    if (surah.surah !== expectedSurah) {
      throw new Error(
        `[quran-source:${sourceProfile}] canonical surah ${surah.surah} at position ${expectedSurah}`,
      );
    }
    for (let ayah = 1; ayah <= surah.ayahCount; ayah += 1) {
      const row = rows[rowIndex];
      const globalIndex = surah.startGlobal + ayah - 1;
      if (
        !row ||
        row.globalIndex !== globalIndex ||
        row.surah !== surah.surah ||
        row.ayah !== ayah
      ) {
        const observed = row ? `${row.globalIndex}/${row.surah}:${row.ayah}` : "missing row";
        throw new Error(
          `[quran-source:${sourceProfile}] coordinate ${observed} != ${globalIndex}/${surah.surah}:${ayah}`,
        );
      }
      rowIndex += 1;
    }
  }
}
