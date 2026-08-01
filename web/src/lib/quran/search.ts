/* ════════════════════════════════════════════════════════════════════════
   search.ts — unified Quran search entry point.

   Precedence: the local sqlite-wasm Worker (offline-capable, full corpus) →
   the live /quran/v1 API (when up) → a synchronous surah name/number fallback
   (no verse text) for before the corpus is ready. All three return the same
   SearchResponse shape; the `source` field says which engine answered.
   ════════════════════════════════════════════════════════════════════════ */

import { QURAN } from "$lib/config/site";
import { CATALOG } from "$lib/data/quran-meta";
import { verseKey } from "$lib/data/quran";
import { quranWorker } from "./worker-client";
import {
  DEFAULT_LIMIT,
  DEFAULT_OFFSET,
  normalizeArabic,
  SearchHitKind,
  SearchProvider,
  type SearchHit,
  type SearchOpts,
  type SearchResponse,
} from "./search/normalize";
import { decodeSearchResponse, unwrapEnvelope } from "./wire";

/** Surah name / Arabic / number fallback when no corpus is available. */
function nameNumberFallback(query: string, opts: SearchOpts): SearchResponse {
  const q = query.trim();
  const qLower = q.toLowerCase();
  const norm = normalizeArabic(q);
  const limit = opts.limit ?? DEFAULT_LIMIT;
  const offset = opts.offset ?? DEFAULT_OFFSET;
  const all: SearchHit[] = [];
  for (const s of CATALOG) {
    const hit =
      s.name.toLowerCase().includes(qLower) ||
      s.arabic.includes(q) ||
      String(s.num) === qLower ||
      (norm.length > 0 && normalizeArabic(s.arabic).includes(norm));
    if (hit)
      all.push({
        kind: SearchHitKind.Ayah,
        key: verseKey(s.num, 1),
        surah: s.num,
        ayah: 1,
        globalIndex: s.startGlobal,
        text: "",
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

/** Search the Quran. Worker-first, then API, then name/number fallback. */
export async function quranSearch(query: string, opts: SearchOpts = {}): Promise<SearchResponse> {
  if (quranWorker.ready) {
    try {
      return await quranWorker.search(query, opts);
    } catch {
      /* fall through */
    }
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
        // Strip the `{ data }` envelope (if any) and rebuild field-by-field via
        // the shared wire decoder — never spread the untrusted API shape. Each
        // hit is re-validated by decodeSearchHit (strict numeric surah/ayah).
        const payload = decodeSearchResponse(unwrapEnvelope(body));
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
    } catch {
      /* fall through */
    }
  }

  return nameNumberFallback(query, opts);
}
