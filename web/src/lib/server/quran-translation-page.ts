import { env } from "$env/dynamic/private";
import { QURAN } from "$lib/config/site";
import { translationIdFromSegments } from "$lib/data/quran";
import type {
  Ayah,
  CatalogEntry,
  QuranRangeText,
  RangePageData,
  SurahLocalPageData,
  SurahRouteContext,
  SurahRouteData,
  SurahNormalization,
} from "$lib/data/quran-types";
import { OpenerKind, OpenerPackaging, QuranScript } from "$lib/data/quran-types";
import { findCatalogueEntry, resolveSourceCatalogue } from "$lib/quran/catalogue";
import {
  FetchHttpError,
  FetchTimeoutError,
  MalformedDataError,
  RANGE_CHUNK_TIMEOUT_MS,
} from "$lib/quran/fetch";
import { fetchRangeChunks, type RangeJsonFetcher } from "$lib/quran/range-fetch";
import { decodeTranslationRangeText, type AyahCoordinateValidator } from "$lib/quran/wire";
import { QURAN_DATA, toSurahRenderMetadata } from "$lib/server/quran-data";
import { requireRangeEntry, surahRouteNav, toRangePageData } from "$lib/server/quran-page-shape";
import { error } from "@sveltejs/kit";

export type TranslationFetcher = (url: string, init?: RequestInit) => Promise<Response>;

function requireApiBase(): string {
  const base = (env.INTERNAL_QURAN_API_BASE || QURAN.apiBase).replace(/\/+$/, "");
  if (!base) {
    throw new Error(
      "[quran-translation] INTERNAL_QURAN_API_BASE or PUBLIC_QURAN_API_BASE not configured",
    );
  }
  return base;
}

function serverJsonFetcher(fetcher: TranslationFetcher): RangeJsonFetcher {
  // Server-only shared secret proving this is trusted Docker-internal SSR to the
  // API's identity gate. Read from private runtime env; never logged, never
  // exposed in a public env var or response, sent only to INTERNAL_QURAN_API_BASE.
  const internalToken = env.INTERNAL_QURAN_API_TOKEN;
  // Only send the secret to the internal Docker base. When INTERNAL_QURAN_API_BASE
  // is unset, requireApiBase() falls back to the public CF-fronted base, which
  // must never receive the shared internal token.
  const usingInternal = !!env.INTERNAL_QURAN_API_BASE;
  return async (url, init) => {
    const ctrl = new AbortController();
    const external = init?.signal;
    const onAbort = (): void => ctrl.abort();
    if (external) {
      if (external.aborted) ctrl.abort();
      else external.addEventListener("abort", onAbort, { once: true });
    }
    const timer = setTimeout(() => ctrl.abort(), init?.timeout ?? RANGE_CHUNK_TIMEOUT_MS);
    try {
      const headers = new Headers(init?.headers);
      if (internalToken && usingInternal) headers.set("x-easyquran-internal-token", internalToken);
      const res = await fetcher(url, { ...init, headers, signal: ctrl.signal });
      if (!res.ok) throw new FetchHttpError(res.status);
      return await res.json();
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") {
        throw external?.aborted ? e : new FetchTimeoutError();
      }
      throw e;
    } finally {
      clearTimeout(timer);
      if (external) external.removeEventListener("abort", onAbort);
    }
  };
}

const validateServerCoordinate: AyahCoordinateValidator = (globalIndex, surah, ayah) =>
  QURAN_DATA.globalIndexOf(surah, ayah) === globalIndex;

async function fetchTranslationRange(
  sourceId: string,
  from: number,
  to: number,
  fetcher: TranslationFetcher,
): Promise<QuranRangeText> {
  try {
    return await fetchRangeChunks({
      base: requireApiBase(),
      source: sourceId,
      from,
      to,
      decode: (raw) => decodeTranslationRangeText(raw, validateServerCoordinate),
      fetchImpl: serverJsonFetcher(fetcher),
    });
  } catch (e) {
    if (e instanceof MalformedDataError) {
      throw new Error(`[quran-translation] malformed range for ${sourceId} ${from}-${to}`);
    }
    if (e instanceof FetchHttpError) {
      throw new Error(`[quran-translation] api ${e.status} for ${sourceId}/range ${from}-${to}`);
    }
    if (e instanceof FetchTimeoutError) {
      throw new Error(`[quran-translation] api timeout for ${sourceId}/range ${from}-${to}`);
    }
    throw e;
  }
}

async function requireTranslationSource(
  sourceId: string,
  lang: string,
  translator: string,
): Promise<void> {
  const catalogue = await resolveSourceCatalogue();
  if (catalogue.length > 0 && !findCatalogueEntry(catalogue, sourceId)) {
    throw error(404, `Unknown translation: ${lang}.${translator}`);
  }
}

function normalizeForSurah(range: QuranRangeText, surah: CatalogEntry): SurahNormalization {
  const normalization = range.normalizations.find((value) => value.surah === surah.num);
  if (
    !normalization ||
    range.ayahs.length === 0 ||
    range.ayahs.some((ayah) => ayah.surah !== surah.num)
  ) {
    throw new Error(`[quran-translation] range does not cover Surah ${surah.num} contiguously`);
  }
  return normalization;
}

function degradedTranslationNormalization(surahNum: number, sourceId: string): SurahNormalization {
  return {
    openerEndScalar: 0,
    bodyStartScalar: 0,
    surah: surahNum,
    sourceId,
    script: QuranScript.Translation,
    sourceProfile: "",
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
  };
}

export async function loadTranslationSurahRouteData(
  surah: CatalogEntry,
  localPage: number,
  lang: string,
  translator: string,
  fetcher: TranslationFetcher,
): Promise<SurahRouteData | undefined> {
  const page = QURAN_DATA.surahLocalPage(surah.num, localPage);
  if (!page) return undefined;
  const sourceId = translationIdFromSegments(lang, translator);
  await requireTranslationSource(sourceId, lang, translator);
  let ayahs: Ayah[];
  let normalization: SurahNormalization;
  let range: QuranRangeText | null;
  try {
    range = await fetchTranslationRange(sourceId, page.startGlobal, page.endGlobal, fetcher);
  } catch {
    range = null;
  }
  if (range) {
    normalization = normalizeForSurah(range, surah);
    ayahs = range.ayahs;
  } else {
    normalization = degradedTranslationNormalization(surah.num, sourceId);
    ayahs = [];
  }
  const pageCount = QURAN_DATA.surahLocalPageCount(surah.num);
  const pageData: SurahLocalPageData = {
    surah: toSurahRenderMetadata(surah),
    page,
    pageCount,
    ayahs,
    normalization,
  };
  const ctx: SurahRouteContext = { kind: "translation", lang, translator };
  return { pageData, ...surahRouteNav(ctx, surah, localPage, pageCount) };
}

export async function loadTranslationRangeData(
  kind: "page" | "juz",
  index: number,
  lang: string,
  translator: string,
  fetcher: TranslationFetcher,
): Promise<RangePageData> {
  const entry = requireRangeEntry(kind, index);
  const sourceId = translationIdFromSegments(lang, translator);
  await requireTranslationSource(sourceId, lang, translator);
  let ayahs: Ayah[];
  let normalizations: SurahNormalization[];
  try {
    const source = await fetchTranslationRange(
      sourceId,
      entry.startGlobal,
      entry.endGlobal,
      fetcher,
    );
    ayahs = source.ayahs;
    normalizations = source.normalizations;
  } catch {
    ayahs = [];
    normalizations = [];
  }
  return toRangePageData(kind, index, entry, ayahs, normalizations);
}
