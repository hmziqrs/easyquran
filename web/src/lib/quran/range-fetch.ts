import type { Ayah, QuranRangeText, SurahNormalization } from "$lib/data/quran-types";

import { MalformedDataError, RESPONSE_CAP, RANGE_CHUNK_TIMEOUT_MS } from "./fetch";
import { unwrapEnvelope } from "./wire";

export type RangeJsonFetcher = (
  url: string,
  init?: RequestInit & { timeout?: number },
) => Promise<unknown>;

export interface RangeFetchOptions {
  base: string;
  source: string;
  from: number;
  to: number;
  decode: (raw: unknown) => QuranRangeText | null;
  fetchImpl: RangeJsonFetcher;
  signal?: AbortSignal;
}

export function planRangeChunks(from: number, to: number): Array<[number, number]> {
  if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to) || from < 1 || to < from) {
    throw new MalformedDataError("invalid range bounds");
  }
  const count = to - from + 1;
  if (count <= RESPONSE_CAP) return [[from, to]];
  if (count > 2 * RESPONSE_CAP) throw new MalformedDataError("range exceeds max chunk count");
  const mid = from + RESPONSE_CAP - 1;
  return [
    [from, mid],
    [mid + 1, to],
  ];
}

function sameNormalization(a: SurahNormalization, b: SurahNormalization): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export function stitchRangeChunks(
  chunks: QuranRangeText[],
  bounds: Array<[number, number]>,
): QuranRangeText | null {
  if (bounds.length === 0) return null;
  const ayahs: Ayah[] = [];
  const normBySurah = new Map<number, SurahNormalization>();
  let expected = bounds[0]![0];
  for (let i = 0; i < chunks.length; i++) {
    const chunk = chunks[i]!;
    const [from, to] = bounds[i]!;
    const want = to - from + 1;
    if (chunk.ayahs.length !== want) return null;
    const first = chunk.ayahs[0];
    const last = chunk.ayahs.at(-1);
    if (!first || !last) return null;
    if (first.globalIndex !== from) return null;
    if (last.globalIndex !== to) return null;
    for (const ayah of chunk.ayahs) {
      if (ayah.globalIndex !== expected) return null;
      ayahs.push(ayah);
      expected++;
    }
    for (const norm of chunk.normalizations) {
      const existing = normBySurah.get(norm.surah);
      if (existing) {
        if (!sameNormalization(existing, norm)) return null;
      } else {
        normBySurah.set(norm.surah, norm);
      }
    }
  }
  const represented = new Set(ayahs.map((a) => a.surah));
  if (represented.size !== normBySurah.size) return null;
  for (const num of normBySurah.keys()) if (!represented.has(num)) return null;
  return { ayahs, normalizations: [...normBySurah.values()] };
}

export async function fetchRangeChunks(opts: RangeFetchOptions): Promise<QuranRangeText> {
  const bounds = planRangeChunks(opts.from, opts.to);
  const chunks: QuranRangeText[] = [];
  for (const [from, to] of bounds) {
    const url = `${opts.base}/sources/${opts.source}/range?from=${from}&to=${to}`;
    const body = await opts.fetchImpl(url, {
      headers: { accept: "application/json" },
      signal: opts.signal,
      timeout: RANGE_CHUNK_TIMEOUT_MS,
    });
    const decoded = opts.decode(unwrapEnvelope(body));
    if (!decoded) throw new MalformedDataError("malformed range chunk");
    chunks.push(decoded);
  }
  const stitched = stitchRangeChunks(chunks, bounds);
  if (!stitched) throw new MalformedDataError("range stitch failed");
  return stitched;
}
