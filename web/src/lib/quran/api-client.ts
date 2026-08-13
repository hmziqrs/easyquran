import { QURAN } from "$lib/config/site";
import { isArabicSourceId } from "$lib/data/quran-types";
import type { QuranRangeText, QuranReaderSource, QuranSurahText } from "$lib/data/quran-types";

import { fetchJsonWithTimeout, MalformedDataError } from "./fetch";
import { fetchRangeChunks } from "./range-fetch";
import { DEFAULT_LIMIT, DEFAULT_OFFSET } from "./search/normalize";
import { SearchProvider, type SearchOpts, type SearchResponse } from "./search/types";
import {
  decodeQuranRangeText,
  decodeQuranSurahText,
  decodeSearchResponse,
  decodeTranslationRangeText,
  decodeTranslationSurahText,
  unwrapEnvelope,
  type AyahCoordinateValidator,
} from "./wire";

function requireBase(): string {
  if (!QURAN.apiBase) throw new Error("quran api base not configured (PUBLIC_QURAN_API_BASE)");
  return QURAN.apiBase;
}

export const quranApi = {
  async readSurah(
    sourceId: QuranReaderSource,
    num: number,
    signal?: AbortSignal,
  ): Promise<QuranSurahText> {
    const body = await fetchJsonWithTimeout(`${requireBase()}/sources/${sourceId}/surah/${num}`, {
      headers: { accept: "application/json" },
      signal,
    });
    const decoded = isArabicSourceId(sourceId)
      ? decodeQuranSurahText(unwrapEnvelope(body))
      : decodeTranslationSurahText(unwrapEnvelope(body));
    if (!decoded) throw new MalformedDataError("malformed surah");
    return decoded;
  },

  async readRange(
    sourceId: QuranReaderSource,
    from: number,
    to: number,
    signal?: AbortSignal,
    validateCoordinate?: AyahCoordinateValidator,
  ): Promise<QuranRangeText> {
    const decode = isArabicSourceId(sourceId)
      ? (raw: unknown) => decodeQuranRangeText(raw, validateCoordinate)
      : (raw: unknown) => decodeTranslationRangeText(raw, validateCoordinate);
    return fetchRangeChunks({
      base: requireBase(),
      source: String(sourceId),
      from,
      to,
      decode,
      fetchImpl: fetchJsonWithTimeout,
      signal,
    });
  },

  async search(
    query: string,
    opts: SearchOpts = {},
    signal?: AbortSignal,
    validateCoordinate?: AyahCoordinateValidator,
  ): Promise<SearchResponse> {
    const url = new URL(`${requireBase()}/search`);
    url.searchParams.set("q", query);
    url.searchParams.set("limit", String(opts.limit ?? DEFAULT_LIMIT));
    url.searchParams.set("offset", String(opts.offset ?? DEFAULT_OFFSET));
    const body = await fetchJsonWithTimeout(url.toString(), {
      headers: { accept: "application/json" },
      signal,
    });
    const payload = decodeSearchResponse(unwrapEnvelope(body), validateCoordinate);
    if (!payload) throw new MalformedDataError("malformed search response");
    return {
      query,
      total: payload.total ?? payload.results.length,
      limit: payload.limit ?? opts.limit ?? DEFAULT_LIMIT,
      offset: payload.offset ?? opts.offset ?? DEFAULT_OFFSET,
      results: payload.results,
      source: SearchProvider.Api,
    };
  },
};
