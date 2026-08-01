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

import {
  isOpenerKind,
  isOpenerPackaging,
  isQuranScript,
  isQuranSourceId,
  OpenerKind,
  type ArtifactSpec,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";
import { SearchHitKind, type SearchHit } from "./search/normalize";
import { sourceProfile } from "./view/source-profiles";

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
  const surah = positiveNumber(rec.surah);
  if (surah === null) return null;
  const highlights = decodeHighlights(rec.highlights);
  if (!highlights) return null;
  if (rec.kind === SearchHitKind.Opener) {
    if (rec.anchorAyah !== 1) return null;
    return {
      kind: SearchHitKind.Opener,
      key: `opener:${surah}`,
      surah,
      anchorAyah: 1,
      text: typeof rec.text === "string" ? rec.text : "",
      highlights,
    };
  }
  if (rec.kind !== SearchHitKind.Ayah) return null;
  const ayah = positiveNumber(rec.ayah);
  if (ayah === null) return null;
  return {
    kind: SearchHitKind.Ayah,
    key: typeof rec.key === "string" ? rec.key : "",
    surah,
    ayah,
    globalIndex: Number(rec.globalIndex) || 0,
    text: typeof rec.text === "string" ? rec.text : "",
    highlights,
  };
}

function positiveNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 1 ? value : null;
}

function nonNegativeNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : null;
}

function decodeHighlights(raw: unknown): { start: number; end: number }[] | null {
  if (!Array.isArray(raw)) return null;
  const out: { start: number; end: number }[] = [];
  for (const item of raw) {
    const rec = asRecord(item);
    if (!rec) return null;
    const start = nonNegativeNumber(rec.start);
    const end = nonNegativeNumber(rec.end);
    if (start === null || end === null || end < start) return null;
    out.push({ start, end });
  }
  return out;
}

/* ── normalized surah Worker payload ─────────────────────────────────── */

export function decodeSurahNormalization(raw: unknown): SurahNormalization | null {
  const rec = asRecord(raw);
  if (!rec || !isQuranSourceId(rec.sourceId) || !isQuranScript(rec.script)) return null;
  if (typeof rec.sourceProfile !== "string" || !isOpenerPackaging(rec.packaging)) {
    return null;
  }
  const profile = sourceProfile(rec.sourceId);
  if (profile.id !== rec.sourceProfile || profile.script !== rec.script) return null;
  if (!isOpenerKind(rec.openerKind)) return null;
  const openerEndScalar = nonNegativeNumber(rec.openerEndScalar);
  const bodyStartScalar = nonNegativeNumber(rec.bodyStartScalar);
  const surah = positiveNumber(rec.surah);
  if (
    surah === null ||
    openerEndScalar === null ||
    bodyStartScalar === null ||
    openerEndScalar > bodyStartScalar
  ) {
    return null;
  }
  const openerText = rec.openerText;
  if (rec.openerKind === OpenerKind.None ? openerText !== null : typeof openerText !== "string") {
    return null;
  }
  return {
    surah,
    sourceId: rec.sourceId,
    script: rec.script,
    sourceProfile: rec.sourceProfile,
    packaging: rec.packaging,
    openerKind: rec.openerKind,
    openerText: openerText as string | null,
    openerEndScalar,
    bodyStartScalar,
  };
}

export function decodeQuranSurahText(raw: unknown): QuranSurahText | null {
  const rec = asRecord(raw);
  if (!rec || !isQuranSourceId(rec.sourceId) || !isQuranScript(rec.script)) return null;
  if (!Array.isArray(rec.verses) || !rec.verses.every((verse) => typeof verse === "string")) {
    return null;
  }
  const normalization = decodeSurahNormalization(rec.normalization);
  if (
    !normalization ||
    normalization.sourceId !== rec.sourceId ||
    normalization.script !== rec.script
  ) {
    return null;
  }
  return {
    sourceId: rec.sourceId,
    script: rec.script,
    verses: rec.verses.slice() as string[],
    normalization,
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
  if (!isQuranSourceId(id)) return null;
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
