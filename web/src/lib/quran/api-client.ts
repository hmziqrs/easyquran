import { QURAN } from "$lib/config/site";
import { isArabicSourceId } from "$lib/data/quran-types";
import type { QuranRangeText, QuranReaderSource, QuranSurahText } from "$lib/data/quran-types";
import { DEFAULT_LIMIT, DEFAULT_OFFSET } from "./search/normalize";
import { SearchProvider, type SearchOpts, type SearchResponse } from "./search/types";
import {
  decodeQuranRangeText,
  decodeQuranSurahText,
  decodeSearchResponse,
  decodeTranslationRangeText,
  decodeTranslationSurahText,
  unwrapEnvelope,
} from "./wire";
import { fetchWithTimeout } from "./fetch";

async function getJson(url: string, signal?: AbortSignal): Promise<unknown> {
  const res = await fetchWithTimeout(url, { headers: { accept: "application/json" }, signal });
  if (!res.ok) throw new Error(`quran api ${res.status}: ${url}`);
  return res.json();
}

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
    const body = await getJson(`${requireBase()}/sources/${sourceId}/surah/${num}`, signal);
    const decoded = isArabicSourceId(sourceId)
      ? decodeQuranSurahText(unwrapEnvelope(body))
      : decodeTranslationSurahText(unwrapEnvelope(body));
    if (!decoded) throw new Error(`quran api: malformed surah for ${sourceId}/${num}`);
    return decoded;
  },

  async readRange(
    sourceId: QuranReaderSource,
    from: number,
    to: number,
    signal?: AbortSignal,
  ): Promise<QuranRangeText> {
    const body = await getJson(
      `${requireBase()}/sources/${sourceId}/range?from=${from}&to=${to}`,
      signal,
    );
    const decoded = isArabicSourceId(sourceId)
      ? decodeQuranRangeText(unwrapEnvelope(body))
      : decodeTranslationRangeText(unwrapEnvelope(body));
    if (!decoded) throw new Error(`quran api: malformed range for ${sourceId}/${from}-${to}`);
    return decoded;
  },

  async search(
    query: string,
    opts: SearchOpts = {},
    signal?: AbortSignal,
  ): Promise<SearchResponse> {
    const url = new URL(`${requireBase()}/search`);
    url.searchParams.set("q", query);
    url.searchParams.set("limit", String(opts.limit ?? DEFAULT_LIMIT));
    url.searchParams.set("offset", String(opts.offset ?? DEFAULT_OFFSET));
    const body = await getJson(url.toString(), signal);
    const payload = decodeSearchResponse(unwrapEnvelope(body));
    if (!payload) throw new Error("quran api: malformed search response");
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
