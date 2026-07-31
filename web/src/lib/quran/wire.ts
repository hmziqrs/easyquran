/* ════════════════════════════════════════════════════════════════════════
   wire.ts — shared handwritten decoders for untrusted Quran payloads.

   Every value that crosses a trust boundary — the /quran/v1 HTTP API
   (search.ts, manifest.ts) and the structuredClone'd worker postMessage
   (worker-client.ts) — arrives as `unknown`. Decoding it was previously
   inlined three times with subtly different strictness. This module defines
   each wire shape exactly once; the consumers reuse these decoders so the
   boundary checks cannot drift apart again.

   Design rules:
     • Handwritten, zero runtime dependencies (no Valibot/Zod). The shapes are
       small and flat; a maintained handwritten decoder is preferred here.
     • Pure functions: `unknown → T | null`. null means "not this shape";
       callers own the fallback policy (DEFAULT_LIMIT, baked manifest, etc.).
     • Always REBUILD field-by-field. We never spread or alias the wire object,
       so a hostile/malformed payload cannot leak unexpected keys or identity
       into the app's domain types.
     • surah/ayah are checked with `typeof === "number"` + finite + >= 1.
       Number() would coerce "", [], or true into a finite 0/1 and emit bogus
       surah=0 hits, so the strict guard is mandatory.

   This module is main-thread only. It imports `./search/normalize` (which is
   itself worker-safe) and `quran-types` (type-only), so it pulls no $env /
   SvelteKit code and could be reused on either side of the worker boundary.
   ════════════════════════════════════════════════════════════════════════ */

import type { ArtifactSpec } from "$lib/data/quran-types";
import type { SearchHit } from "./search/normalize";

/** Narrow an unknown to a string-keyed record, or null. */
function asRecord(raw: unknown): Record<string, unknown> | null {
  return raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
}

/* ── envelope ─────────────────────────────────────────────────────────── */

/** Strip a `{ data: ... }` envelope, returning the inner payload when `data`
 *  is present and non-nullish, otherwise the body itself. Both /search and
 *  /version + /scripts may be enveloped or bare depending on the backend build;
 *  this normalizes the two. Uses `??` (not `||`) so a falsy-but-present `data`
 *  (e.g. `0`, `""`, `false`) is preserved exactly as the prior inline code did. */
export function unwrapEnvelope(raw: unknown): unknown {
  const rec = asRecord(raw);
  if (!rec) return raw;
  return rec.data ?? raw;
}

/* ── search ───────────────────────────────────────────────────────────── */

/** Strictly decode one search hit from an untrusted source (API JSON or worker
 *  postMessage). Rebuilds field-by-field. Returns null when surah/ayah are
 *  absent, non-numeric, non-finite, or out of range — never coerces them. */
export function decodeSearchHit(raw: unknown): SearchHit | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const { surah, ayah } = rec;
  // Require real numbers in range — Number() would otherwise coerce "", [],
  // or true into finite 0/1 and emit bogus surah=0 hits.
  if (
    typeof surah !== "number" ||
    typeof ayah !== "number" ||
    !Number.isFinite(surah) ||
    !Number.isFinite(ayah) ||
    surah < 1 ||
    ayah < 1
  ) {
    return null;
  }
  return {
    key: typeof rec.key === "string" ? rec.key : "",
    surah,
    ayah,
    globalIndex: Number(rec.globalIndex) || 0,
    text: typeof rec.text === "string" ? rec.text : "",
  };
}

/** The validated scalar fields of a SearchResponse, with the `results` array
 *  rebuilt. Each scalar is the wire value when it is a finite number / string,
 *  else null — callers decide the fallback. `query` is included so an API
 *  consumer can echo it back when it differs from the request. */
export interface DecodedSearchPayload {
  query: string | null;
  total: number | null;
  limit: number | null;
  offset: number | null;
  results: SearchHit[];
}

/** Decode the common SearchResponse wire shape shared by the /search API
 *  response and the worker's `search` RPC reply. Callers strip the envelope
 *  first (the API wraps in `{ data }`; the worker does not) via
 *  {@link unwrapEnvelope}. Returns null when the payload is not an object or
 *  `results` is missing/not an array. Individual malformed hits are dropped
 *  (not fatal) so a single bad row cannot blank a whole worker result set. */
export function decodeSearchResponse(raw: unknown): DecodedSearchPayload | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  if (!Array.isArray(rec.results)) return null;
  const results: SearchHit[] = [];
  for (const item of rec.results) {
    const hit = decodeSearchHit(item);
    if (hit) results.push(hit);
  }
  return {
    query: typeof rec.query === "string" ? rec.query : null,
    total: typeof rec.total === "number" && Number.isFinite(rec.total) ? rec.total : null,
    limit: typeof rec.limit === "number" && Number.isFinite(rec.limit) ? rec.limit : null,
    offset: typeof rec.offset === "number" && Number.isFinite(rec.offset) ? rec.offset : null,
    results,
  };
}

/* ── manifest (/version + /scripts) ───────────────────────────────────── */

/** Decode one /scripts entry → ArtifactSpec. Mirrors the prior `normalizeScript`
 *  exactly: `id` must be a known script, `sizeBytes` must coerce to a truthy
 *  finite number, and `sha256` / `downloadUrl` must be strings. */
export function decodeScript(raw: unknown): ArtifactSpec | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const id = rec.id;
  if (id !== "uthmani" && id !== "simple-clean") return null;
  const sizeBytes = Number(rec.sizeBytes);
  const sha256 = rec.sha256;
  const downloadUrl = rec.downloadUrl;
  if (!sizeBytes || typeof sha256 !== "string" || typeof downloadUrl !== "string") return null;
  return { id, sizeBytes, sha256, downloadUrl };
}

/** Decode a full /scripts response body (envelope-stripped) into the validated
 *  artifact list. Returns null when the body or its `scripts` field is not the
 *  expected shape; invalid entries are dropped. Callers check `length < 2` to
 *  decide whether to fall back to baked config. */
export function decodeScriptsPayload(rawBody: unknown): ArtifactSpec[] | null {
  const data = asRecord(unwrapEnvelope(rawBody));
  if (!data) return null;
  if (!Array.isArray(data.scripts)) return null;
  const out: ArtifactSpec[] = [];
  for (const item of data.scripts) {
    const spec = decodeScript(item);
    if (spec) out.push(spec);
  }
  return out;
}

/** The validated fields of a /version response. Each is the wire string when
 *  present, else null — the manifest caller falls back to the baked config. */
export interface DecodedVersion {
  contentVersion: string | null;
  searchVersion: string | null;
}

/** Decode a /version response body (envelope-stripped) into its version
 *  strings. Returns null only when the body is not an object; missing or
 *  non-string fields are surfaced as null so the caller can apply defaults
 *  without rejecting an otherwise-usable response. */
export function decodeVersionPayload(rawBody: unknown): DecodedVersion | null {
  const data = asRecord(unwrapEnvelope(rawBody));
  if (!data) return null;
  return {
    contentVersion: typeof data.contentVersion === "string" ? data.contentVersion : null,
    searchVersion: typeof data.searchVersion === "string" ? data.searchVersion : null,
  };
}
