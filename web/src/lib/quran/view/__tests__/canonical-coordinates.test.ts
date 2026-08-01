import { describe, expect, it } from "vite-plus/test";
import {
  CANONICAL_QURAN_COORDINATES,
  isCanonicalAyahCoordinate,
  validateCanonicalCoordinates,
} from "$lib/quran/view/canonical-coordinates";
import type { QuranCoordinateRow } from "$lib/quran/sql";

function canonicalRows(): QuranCoordinateRow[] {
  return CANONICAL_QURAN_COORDINATES.surahs.flatMap((surah) =>
    Array.from({ length: surah.ayahCount }, (_, index) => ({
      globalIndex: surah.startGlobal + index,
      surah: surah.surah,
      ayah: index + 1,
    })),
  );
}

describe("canonical source-coordinate validation", () => {
  it("accepts every quran-data.xml coordinate", () => {
    const rows = canonicalRows();
    expect(rows).toHaveLength(6236);
    expect(() => validateCanonicalCoordinates("fixture", rows)).not.toThrow();
  });

  it("validates individual wire coordinates against the same projection", () => {
    expect(isCanonicalAyahCoordinate(262, 2, 255)).toBe(true);
    expect(isCanonicalAyahCoordinate(263, 2, 255)).toBe(false);
    expect(isCanonicalAyahCoordinate(294, 2, 287)).toBe(false);
  });

  it("fails closed on a shifted or duplicated source key", () => {
    const rows = canonicalRows();
    rows[7] = { ...rows[7]!, ayah: 2 };
    expect(() => validateCanonicalCoordinates("fixture", rows)).toThrow(
      "coordinate 8/2:2 != 8/2:1",
    );
  });
});
