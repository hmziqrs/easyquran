import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import type { Ayah, QuranRangeText, SurahNormalization } from "$lib/data/quran-types";
import { MalformedDataError, RESPONSE_CAP, RANGE_CHUNK_TIMEOUT_MS } from "$lib/quran/fetch";
import {
  fetchRangeChunks,
  planRangeChunks,
  stitchRangeChunks,
  type RangeJsonFetcher,
} from "$lib/quran/range-fetch";

function norm(surah: number): SurahNormalization {
  return {
    surah,
    sourceId: "en.x",
    script: QuranScript.Translation,
    sourceProfile: "translation-profile",
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
}

function ayahs(from: number, to: number, surahBase = 1): Ayah[] {
  const out: Ayah[] = [];
  for (let g = from; g <= to; g++) {
    const surah = surahBase + Math.floor((g - from) / 7);
    const ayah = ((g - from) % 7) + 1;
    out.push({ key: `${surah}:${ayah}`, surah, ayah, globalIndex: g, text: `t${g}` });
  }
  return out;
}

function chunk(from: number, to: number, surahBase = 1): QuranRangeText {
  const list = ayahs(from, to, surahBase);
  const bySurah = new Map<number, SurahNormalization>();
  for (const a of list) bySurah.set(a.surah, norm(a.surah));
  return { ayahs: list, normalizations: [...bySurah.values()] };
}

function fetcherReturning(...bodies: unknown[]): RangeJsonFetcher {
  const queue = [...bodies];
  return vi.fn(async () => {
    const next = queue.shift();
    if (next === undefined) throw new Error("fetcher exhausted");
    if (next instanceof Error) throw next;
    return next;
  }) as unknown as RangeJsonFetcher;
}

const decoder = (raw: unknown): QuranRangeText | null => {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  if (!Array.isArray(r.ayahs) || !Array.isArray(r.normalizations)) return null;
  return raw as QuranRangeText;
};

describe("planRangeChunks", () => {
  it("returns a single chunk at or under the cap", () => {
    expect(planRangeChunks(1, RESPONSE_CAP)).toEqual([[1, RESPONSE_CAP]]);
    expect(planRangeChunks(1, 10)).toEqual([[1, 10]]);
  });

  it("splits every oversized juz into exactly two cap-sized-or-below chunks", () => {
    const cases: Array<[number, number, number]> = [
      [2876, 3214, 339],
      [3733, 4089, 357],
      [4706, 5104, 399],
      [5242, 5672, 431],
      [5673, 6236, 564],
    ];
    for (const [from, to, count] of cases) {
      const chunks = planRangeChunks(from, to);
      expect(chunks).toHaveLength(2);
      expect(chunks[0]![0]).toBe(from);
      expect(chunks.at(-1)![1]).toBe(to);
      for (const [f, t] of chunks) {
        expect(t - f + 1).toBeLessThanOrEqual(RESPONSE_CAP);
      }
      expect(chunks.reduce((sum, [f, t]) => sum + (t - f + 1), 0)).toBe(count);
      expect(chunks[1]![0]).toBe(chunks[0]![1] + 1);
    }
  });

  it("rejects a range that would need more than two chunks", () => {
    expect(() => planRangeChunks(1, 2 * RESPONSE_CAP + 1)).toThrow(MalformedDataError);
  });

  it("rejects inverted or non-integer bounds", () => {
    expect(() => planRangeChunks(5, 4)).toThrow(MalformedDataError);
    expect(() => planRangeChunks(0, 3)).toThrow(MalformedDataError);
    expect(() => planRangeChunks(1.5, 3)).toThrow(MalformedDataError);
  });
});

describe("stitchRangeChunks", () => {
  it("returns the single chunk unchanged when bounds match", () => {
    const c = chunk(1, 5);
    expect(stitchRangeChunks([c], [[1, 5]])).toEqual(c);
  });

  it("stitches two adjacent chunks and concatenates ayahs", () => {
    const a = chunk(1, 3);
    const b = chunk(4, 6);
    const stitched = stitchRangeChunks(
      [a, b],
      [
        [1, 3],
        [4, 6],
      ],
    );
    expect(stitched?.ayahs.map((x) => x.globalIndex)).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("merges byte-equivalent normalization duplicates from a split inside one surah", () => {
    const a: QuranRangeText = { ayahs: ayahs(1, 4, 1), normalizations: [norm(1)] };
    const b: QuranRangeText = { ayahs: ayahs(5, 7, 1), normalizations: [norm(1)] };
    const stitched = stitchRangeChunks(
      [a, b],
      [
        [1, 4],
        [5, 7],
      ],
    );
    expect(stitched?.normalizations).toEqual([norm(1)]);
    expect(stitched?.ayahs).toHaveLength(7);
  });

  it("rejects conflicting duplicate normalizations for the same surah", () => {
    const a: QuranRangeText = { ayahs: ayahs(1, 4, 1), normalizations: [norm(1)] };
    const conflicting: SurahNormalization = { ...norm(1), sourceId: "en.other" };
    const b: QuranRangeText = { ayahs: ayahs(5, 7, 1), normalizations: [conflicting] };
    expect(
      stitchRangeChunks(
        [a, b],
        [
          [1, 4],
          [5, 7],
        ],
      ),
    ).toBeNull();
  });

  it("rejects a non-adjacent second chunk", () => {
    const a = chunk(1, 3);
    const b = chunk(5, 7);
    expect(
      stitchRangeChunks(
        [a, b],
        [
          [1, 3],
          [5, 7],
        ],
      ),
    ).toBeNull();
  });

  it("rejects a chunk whose first/last indices disagree with bounds", () => {
    const shifted: QuranRangeText = {
      ayahs: ayahs(2, 6),
      normalizations: [norm(1), norm(2)],
    };
    expect(stitchRangeChunks([shifted], [[1, 5]])).toBeNull();
  });

  it("rejects a chunk with the wrong count", () => {
    const shortChunk: QuranRangeText = { ayahs: ayahs(1, 4), normalizations: [norm(1)] };
    expect(stitchRangeChunks([shortChunk], [[1, 5]])).toBeNull();
  });

  it("rejects when normalizations cover a surah absent from ayahs", () => {
    const bad: QuranRangeText = {
      ayahs: ayahs(1, 3, 1),
      normalizations: [norm(1), norm(2)],
    };
    expect(stitchRangeChunks([bad], [[1, 3]])).toBeNull();
  });
});

describe("fetchRangeChunks", () => {
  afterEach(() => vi.restoreAllMocks());

  it("issues one request for an in-cap range and returns the stitched result", async () => {
    const f = fetcherReturning({ data: chunk(1, 5) });
    const result = await fetchRangeChunks({
      base: "https://x/api",
      source: "en.x",
      from: 1,
      to: 5,
      decode: decoder,
      fetchImpl: f,
    });
    expect(result.ayahs.map((a) => a.globalIndex)).toEqual([1, 2, 3, 4, 5]);
    expect(f).toHaveBeenCalledTimes(1);
    const [url, init] = (f as unknown as ReturnType<typeof vi.fn>).mock.calls[0]!;
    expect(url).toBe("https://x/api/sources/en.x/range?from=1&to=5");
    expect((init as RequestInit & { timeout?: number }).timeout).toBe(RANGE_CHUNK_TIMEOUT_MS);
  });

  it("chunks an oversized range into two requests and stitches them", async () => {
    const mid = 1 + RESPONSE_CAP - 1;
    const f = fetcherReturning({ data: chunk(1, mid) }, { data: chunk(mid + 1, mid + 50) });
    const result = await fetchRangeChunks({
      base: "https://x/api",
      source: "en.x",
      from: 1,
      to: mid + 50,
      decode: decoder,
      fetchImpl: f,
    });
    expect(f).toHaveBeenCalledTimes(2);
    expect(result.ayahs).toHaveLength(RESPONSE_CAP + 50);
    expect(result.ayahs[0]!.globalIndex).toBe(1);
    expect(result.ayahs.at(-1)!.globalIndex).toBe(mid + 50);
  });

  it("rejects the whole read when one chunk fails, with no partial result", async () => {
    const mid = 1 + RESPONSE_CAP - 1;
    const f = fetcherReturning({ data: chunk(1, mid) }, new Error("boom"));
    await expect(
      fetchRangeChunks({
        base: "https://x/api",
        source: "en.x",
        from: 1,
        to: mid + 50,
        decode: decoder,
        fetchImpl: f,
      }),
    ).rejects.toThrow(/boom/);
  });

  it("rejects the whole read when stitching detects a non-adjacent second chunk", async () => {
    const mid = 1 + RESPONSE_CAP - 1;
    const nonAdjacent: QuranRangeText = {
      ayahs: ayahs(mid + 2, mid + 51, 1),
      normalizations: [norm(1)],
    };
    const f = fetcherReturning({ data: chunk(1, mid) }, { data: nonAdjacent });
    await expect(
      fetchRangeChunks({
        base: "https://x/api",
        source: "en.x",
        from: 1,
        to: mid + 50,
        decode: decoder,
        fetchImpl: f,
      }),
    ).rejects.toThrow(MalformedDataError);
  });

  it("rejects when a chunk payload is malformed", async () => {
    const f = fetcherReturning({ data: { ayahs: "no", normalizations: [] } });
    await expect(
      fetchRangeChunks({
        base: "https://x/api",
        source: "en.x",
        from: 1,
        to: 5,
        decode: decoder,
        fetchImpl: f,
      }),
    ).rejects.toThrow(MalformedDataError);
  });
});

// W5: the five oversized juz (19/23/27/29/30) are the only ranges that exceed
// the backend RESPONSE_CAP. They must pass through the shared server/SSR range
// path (fetchRangeChunks, used by quran-translation-page.ts) as at most two
// cap-sized requests that stitch back to one contiguous result.
describe("five oversized juz through the shared range path", () => {
  afterEach(() => vi.restoreAllMocks());

  // [juz, from, to] — global-index bounds and the exact ayah count each juz
  // covers, computed from web/static/quran-meta/quran-data.json.
  const JUZ: Array<[number, number, number]> = [
    [19, 2876, 3214],
    [23, 3733, 4089],
    [27, 4706, 5104],
    [29, 5242, 5672],
    [30, 5673, 6236],
  ];

  // Build a contiguous chunk whose surah assignment increases monotonically
  // with the global index (surah every 50 ayahs), so every represented surah
  // carries exactly one normalization.
  function contiguousChunk(from: number, to: number): QuranRangeText {
    const list: Ayah[] = [];
    const bySurah = new Map<number, SurahNormalization>();
    for (let g = from; g <= to; g++) {
      const surah = Math.floor((g - 1) / 50) + 1;
      list.push({ key: `${surah}:${g}`, surah, ayah: g, globalIndex: g, text: `t${g}` });
      bySurah.set(surah, norm(surah));
    }
    return { ayahs: list, normalizations: [...bySurah.values()] };
  }

  it.each(JUZ)("juz %i chunks to <=2 cap-sized requests and stitches contiguously", async (juz, from, to) => {
    void juz;
    const count = to - from + 1;
    expect(count).toBeGreaterThan(RESPONSE_CAP); // sanity: this juz is oversized

    const bounds = planRangeChunks(from, to);
    const bodies = bounds.map(([f, t]) => ({ data: contiguousChunk(f, t) }));
    const f = fetcherReturning(...bodies);

    const result = await fetchRangeChunks({
      base: "https://x/api",
      source: "en.x",
      from,
      to,
      decode: decoder,
      fetchImpl: f,
    });

    // Out-of-cap request chunks to at most 2, each at most RESPONSE_CAP wide.
    const calls = (f as unknown as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls).toHaveLength(bounds.length);
    expect(bounds.length).toBeLessThanOrEqual(2);
    for (const [bf, bt] of bounds) {
      expect(bt - bf + 1).toBeLessThanOrEqual(RESPONSE_CAP);
    }

    // Stitched result covers [from,to] exactly and contiguously.
    expect(result.ayahs).toHaveLength(count);
    expect(result.ayahs[0]!.globalIndex).toBe(from);
    expect(result.ayahs.at(-1)!.globalIndex).toBe(to);
    for (let i = 1; i < result.ayahs.length; i++) {
      expect(result.ayahs[i]!.globalIndex).toBe(result.ayahs[i - 1]!.globalIndex + 1);
    }
  });

  it("merges byte-equivalent normalization when the chunk split falls inside one surah", async () => {
    // Force the planner's split point to land inside surah 1: both chunks
    // contain only surah-1 ayahs and both return the same normalization, so
    // stitching must dedupe to one surah-1 normalization entry.
    const from = 1;
    const to = RESPONSE_CAP + 5;
    const bounds = planRangeChunks(from, to);
    expect(bounds).toHaveLength(2);
    const oneSurahAyahs = (a: number, b: number): Ayah[] => {
      const out: Ayah[] = [];
      for (let g = a; g <= b; g++) out.push({ key: `1:${g}`, surah: 1, ayah: g, globalIndex: g, text: `t${g}` });
      return out;
    };
    const a: QuranRangeText = { ayahs: oneSurahAyahs(bounds[0]![0], bounds[0]![1]), normalizations: [norm(1)] };
    const b: QuranRangeText = { ayahs: oneSurahAyahs(bounds[1]![0], bounds[1]![1]), normalizations: [norm(1)] };
    const f = fetcherReturning({ data: a }, { data: b });

    const result = await fetchRangeChunks({
      base: "https://x/api",
      source: "en.x",
      from,
      to,
      decode: decoder,
      fetchImpl: f,
    });

    expect(result.ayahs).toHaveLength(to - from + 1);
    expect(result.normalizations).toEqual([norm(1)]);
    expect(result.normalizations).toHaveLength(1);
  });
});
