import { describe, expect, it, vi } from "vite-plus/test";
import { RangeKind } from "$lib/data/quran-data";
import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import type { Ayah } from "$lib/data/quran-types";
import { RESPONSE_CAP } from "$lib/quran/fetch";
import { QURAN_DATA } from "$lib/server/quran-data";
import { loadTranslationRangeData } from "$lib/server/quran-translation-page";

vi.mock("$env/dynamic/private", () => ({
  env: { INTERNAL_QURAN_API_BASE: "https://api.test", INTERNAL_QURAN_API_TOKEN: undefined },
}));

vi.mock("$env/dynamic/public", () => ({
  env: { PUBLIC_QURAN_API_BASE: "", PUBLIC_ENV: "local" },
}));

vi.mock("$lib/quran/catalogue", () => ({
  resolveSourceCatalogue: vi.fn(async () => []),
  findCatalogueEntry: vi.fn(() => undefined),
}));

const SOURCE_ID = "en.sahih";

function translationNorm(surah: number) {
  return {
    surah,
    sourceId: SOURCE_ID,
    script: QuranScript.Translation,
    sourceProfile: `translation:${SOURCE_ID}`,
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
}

// Build a wire envelope for [from,to] using the canonical surah map, so every
// represented surah is real and decodeTranslationRangeText accepts the chunk.
function wireChunk(from: number, to: number) {
  const ayahs: Ayah[] = [];
  const bySurah = new Set<number>();
  for (let g = from; g <= to; g++) {
    const verseKey = QURAN_DATA.verseKeyAtGlobal(g)!;
    const [surah, ayah] = verseKey.split(":").map(Number);
    ayahs.push({ key: verseKey, surah, ayah, globalIndex: g, text: `t${g}` });
    bySurah.add(surah);
  }
  const normalizations = [...bySurah].map((s) => translationNorm(s));
  return { data: { ayahs, normalizations } };
}

function makeFetcher() {
  const calls: Array<{ from: number; to: number }> = [];
  const fetcher = vi.fn(async (url: string) => {
    const u = new URL(url);
    const from = Number(u.searchParams.get("from"));
    const to = Number(u.searchParams.get("to"));
    calls.push({ from, to });
    return {
      ok: true,
      status: 200,
      json: async () => wireChunk(from, to),
    } as Response;
  });
  return { fetcher, calls };
}

describe("loadTranslationRangeData juz chunking through the SSR loader", () => {
  it.each([19, 23, 27, 29, 30])(
    "juz %i (oversized) chunks to <=2 cap-sized requests with no truncation",
    async (juz) => {
      const entry = QURAN_DATA.rangeByIndex(RangeKind.Juz, juz)!;
      const count = entry.endGlobal - entry.startGlobal + 1;
      expect(count).toBeGreaterThan(RESPONSE_CAP); // sanity: this juz is oversized

      const { fetcher, calls } = makeFetcher();
      const result = await loadTranslationRangeData("juz", juz, "en", "sahih", fetcher);

      // The loader hit the upstream range endpoint, split across at most two
      // cap-sized requests that together cover the whole juz.
      expect(calls).toHaveLength(2);
      for (const { from, to } of calls) {
        expect(to - from + 1).toBeLessThanOrEqual(RESPONSE_CAP);
      }
      expect(calls[0]!.from).toBe(entry.startGlobal);
      expect(calls.at(-1)!.to).toBe(entry.endGlobal);
      expect(calls[1]!.from).toBe(calls[0]!.to + 1);

      // Complete, contiguous, untruncated data stitched back together.
      expect(result.kind).toBe("juz");
      expect(result.index).toBe(juz);
      expect(result.startGlobal).toBe(entry.startGlobal);
      expect(result.endGlobal).toBe(entry.endGlobal);
      expect(result.ayahs).toHaveLength(count);
      expect(result.ayahs[0]!.globalIndex).toBe(entry.startGlobal);
      expect(result.ayahs.at(-1)!.globalIndex).toBe(entry.endGlobal);
      for (let i = 1; i < result.ayahs.length; i++) {
        expect(result.ayahs[i]!.globalIndex).toBe(result.ayahs[i - 1]!.globalIndex + 1);
      }

      // Every represented surah carries exactly one normalization.
      const represented = new Set(result.ayahs.map((a) => a.surah));
      expect(result.normalizations).toHaveLength(represented.size);
      expect(result.surahs.map((s) => s.num).sort((a, b) => a - b)).toEqual(
        [...represented].sort((a, b) => a - b),
      );
    },
  );

  it("serves an in-cap juz as a single unchunked request", async () => {
    const entry = QURAN_DATA.rangeByIndex(RangeKind.Juz, 1)!;
    expect(entry.endGlobal - entry.startGlobal + 1).toBeLessThanOrEqual(RESPONSE_CAP);

    const { fetcher, calls } = makeFetcher();
    const result = await loadTranslationRangeData("juz", 1, "en", "sahih", fetcher);

    expect(calls).toHaveLength(1);
    expect(result.ayahs).toHaveLength(entry.endGlobal - entry.startGlobal + 1);
    expect(result.ayahs[0]!.globalIndex).toBe(entry.startGlobal);
    expect(result.ayahs.at(-1)!.globalIndex).toBe(entry.endGlobal);
  });
});

describe("SSR translation range per-chunk coordinate validation", () => {
  // Corrupt one ayah's ayah-number (and its key to stay self-consistent) while
  // leaving its globalIndex and surah intact. The represented-surah set and
  // contiguity are unchanged, so without a coordinate validator this decodes
  // fine; the server validator must reject the whole read.
  function wireChunkBadCoordinate(from: number, to: number) {
    const chunk = wireChunk(from, to);
    const body = chunk.data;
    const target = body.ayahs[1]!;
    const wrongAyah = target.ayah + 5;
    body.ayahs[1] = { ...target, ayah: wrongAyah, key: `${target.surah}:${wrongAyah}` };
    return body;
  }

  it("rejects a chunk whose (surah,ayah) disagrees with globalIndex (no partial render)", async () => {
    const entry = QURAN_DATA.rangeByIndex(RangeKind.Juz, 1)!;
    const body = wireChunkBadCoordinate(entry.startGlobal, entry.endGlobal);

    const fetcher = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ data: { ayahs: body.ayahs, normalizations: body.normalizations } }),
    } as Response));

    const result = await loadTranslationRangeData("juz", 1, "en", "sahih", fetcher);

    expect(result.ayahs).toHaveLength(0);
    expect(result.normalizations).toHaveLength(0);
  });

  it("accepts an otherwise-identical chunk whose coordinates agree with globalIndex", async () => {
    const entry = QURAN_DATA.rangeByIndex(RangeKind.Juz, 1)!;
    const { fetcher } = makeFetcher();
    const result = await loadTranslationRangeData("juz", 1, "en", "sahih", fetcher);
    expect(result.ayahs).toHaveLength(entry.endGlobal - entry.startGlobal + 1);
  });
});
