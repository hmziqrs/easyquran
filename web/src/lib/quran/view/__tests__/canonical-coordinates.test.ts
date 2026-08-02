import { describe, expect, it } from "vite-plus/test";
import {
  isCanonicalAyahCoordinate,
  validateCanonicalCoordinates,
} from "$lib/quran/view/canonical-coordinates";
import type { QuranCoordinateRow } from "$lib/quran/sql";
import { QURAN_CATALOG } from "$lib/server/quran-metadata";

const CANONICAL_QURAN_COORDINATES = QURAN_CATALOG.coordinates;

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
  it("accepts every immutable catalog coordinate", () => {
    const rows = canonicalRows();
    expect(rows).toHaveLength(6236);
    expect(() =>
      validateCanonicalCoordinates("fixture", rows, CANONICAL_QURAN_COORDINATES),
    ).not.toThrow();
  });

  it("validates individual wire coordinates against the same projection", () => {
    expect(isCanonicalAyahCoordinate(262, 2, 255, CANONICAL_QURAN_COORDINATES)).toBe(true);
    expect(isCanonicalAyahCoordinate(263, 2, 255, CANONICAL_QURAN_COORDINATES)).toBe(false);
    expect(isCanonicalAyahCoordinate(294, 2, 287, CANONICAL_QURAN_COORDINATES)).toBe(false);
  });

  it("fails closed on a shifted or duplicated source key", () => {
    const rows = canonicalRows();
    rows[7] = { ...rows[7]!, ayah: 2 };
    expect(() =>
      validateCanonicalCoordinates("fixture", rows, CANONICAL_QURAN_COORDINATES),
    ).toThrow("coordinate 8/2:2 != 8/2:1");
  });
});
