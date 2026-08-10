import { describe, expect, it, vi } from "vite-plus/test";
import type { QuranCoordinateRow } from "$lib/quran/sql";
import { QuranSourceId } from "$lib/data/quran-types";
import { QURAN_ROW_COUNT } from "$lib/workers/opfs-cache";

// quran.worker.ts statically imports resolveSourceProfile, so the mock must be
// in place before the worker module first loads. Default to the real resolver;
// the profile-mismatch test flips the override to a short row count.
const profileOverride = vi.hoisted(() => ({ value: null as { canonicalRowCount: number } | null }));
vi.mock("$lib/quran/view/source-profiles", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/quran/view/source-profiles")>();
  return {
    ...actual,
    resolveSourceProfile: (id: QuranSourceId) =>
      profileOverride.value ?? actual.resolveSourceProfile(id),
  };
});

async function realValidator() {
  const mod = await import("$lib/workers/quran.worker");
  return mod.assertStagedQuranContent;
}

function validRows(): QuranCoordinateRow[] {
  const rows: QuranCoordinateRow[] = [];
  for (let i = 1; i <= QURAN_ROW_COUNT; i++) {
    rows.push({ globalIndex: i, surah: 1, ayah: i });
  }
  return rows;
}

describe("assertStagedQuranContent content checks", () => {
  it("accepts a representative valid staged payload for an Arabic source", async () => {
    const validate = await realValidator();
    expect(() => validate(QURAN_ROW_COUNT, validRows(), QuranSourceId.TanzilUthmani)).not.toThrow();
  });

  it("accepts a valid payload for a translation source (profile check skipped)", async () => {
    const validate = await realValidator();
    expect(() => validate(QURAN_ROW_COUNT, validRows(), "en.sahih")).not.toThrow();
  });

  it("rejects a tampered row count", async () => {
    const validate = await realValidator();
    const rows = validRows();
    expect(() => validate(QURAN_ROW_COUNT - 1, rows, QuranSourceId.TanzilUthmani)).toThrow(
      /row count/,
    );
  });

  it("rejects a coordinate count that disagrees with the row count", async () => {
    const validate = await realValidator();
    const short = validRows().slice(0, QURAN_ROW_COUNT - 1);
    expect(() => validate(QURAN_ROW_COUNT, short, QuranSourceId.TanzilUthmani)).toThrow(
      /coordinate count/,
    );
  });

  it("rejects non-contiguous globalIndex coordinates", async () => {
    const validate = await realValidator();
    const rows = validRows();
    rows[100] = { globalIndex: rows[100]!.globalIndex + 5, surah: 1, ayah: 101 };
    expect(() => validate(QURAN_ROW_COUNT, rows, QuranSourceId.TanzilUthmani)).toThrow(
      /non-contiguous/,
    );
  });

  it("rejects a bad coordinate (surah out of range)", async () => {
    const validate = await realValidator();
    const rows = validRows();
    rows[5] = { globalIndex: 6, surah: 0, ayah: 6 };
    expect(() => validate(QURAN_ROW_COUNT, rows, QuranSourceId.TanzilUthmani)).toThrow(
      /bad coordinate/,
    );
  });

  it("rejects an Arabic source whose profile row count disagrees", async () => {
    profileOverride.value = { canonicalRowCount: QURAN_ROW_COUNT - 1 };
    try {
      const validate = await realValidator();
      expect(() =>
        validate(QURAN_ROW_COUNT, validRows(), QuranSourceId.TanzilUthmani),
      ).toThrow(/profile row count mismatch/);
    } finally {
      profileOverride.value = null;
    }
  });
});
