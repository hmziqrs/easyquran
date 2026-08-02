import { QURAN } from "$lib/config/site";
import { loadQuranData } from "$lib/data/quran-data-client";
import { verseKey } from "$lib/data/quran";
import { quranWorker } from "./worker-client";
import { DEFAULT_LIMIT, DEFAULT_OFFSET, normalizeArabic } from "./search/normalize";
import {
  SearchHitKind,
  SearchProvider,
  type SearchHit,
  type SearchOpts,
  type SearchResponse,
} from "./search/types";
import { decodeSearchResponse, unwrapEnvelope } from "./wire";

async function nameNumberFallback(query: string, opts: SearchOpts): Promise<SearchResponse> {
  const catalog = await loadQuranData();
  const q = query.trim();
  const qLower = q.toLowerCase();
  const norm = normalizeArabic(q);
  const limit = opts.limit ?? DEFAULT_LIMIT;
  const offset = opts.offset ?? DEFAULT_OFFSET;
  const all: SearchHit[] = [];
  for (const s of catalog.surahs) {
    const hit =
      s.name.toLowerCase().includes(qLower) ||
      s.arabic.includes(q) ||
      String(s.num) === qLower ||
      (norm.length > 0 && normalizeArabic(s.arabic).includes(norm));
    if (hit)
      all.push({
        kind: SearchHitKind.Ayah,
        ayah: {
          key: verseKey(s.num, 1),
          surah: s.num,
          ayah: 1,
          globalIndex: s.startGlobal,
          text: "",
        },
        highlights: [],
      });
  }
  return {
    query,
    total: all.length,
    limit,
    offset,
    results: all.slice(offset, offset + limit),
    source: SearchProvider.Names,
  };
}

export async function quranSearch(query: string, opts: SearchOpts = {}): Promise<SearchResponse> {
  const catalog = await loadQuranData();
  const validateCoordinate = (globalIndex: number, surah: number, ayah: number): boolean =>
    catalog.globalIndexOf(surah, ayah) === globalIndex;
  if (quranWorker.ready) {
    try {
      return await quranWorker.search(query, opts, validateCoordinate);
    } catch {}
  }

  if (QURAN.apiBase) {
    try {
      const url = new URL(`${QURAN.apiBase}/search`);
      url.searchParams.set("q", query);
      url.searchParams.set("limit", String(opts.limit ?? DEFAULT_LIMIT));
      url.searchParams.set("offset", String(opts.offset ?? DEFAULT_OFFSET));
      const res = await fetch(url, { headers: { accept: "application/json" } });
      if (res.ok) {
        const body = await res.json();
        const payload = decodeSearchResponse(unwrapEnvelope(body), validateCoordinate);
        if (payload) {
          return {
            query,
            total: payload.total ?? payload.results.length,
            limit: payload.limit || DEFAULT_LIMIT,
            offset: payload.offset || DEFAULT_OFFSET,
            results: payload.results,
            source: SearchProvider.Api,
          };
        }
      }
    } catch {}
  }

  return nameNumberFallback(query, opts);
}
