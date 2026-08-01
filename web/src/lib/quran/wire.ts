/*
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

   This module is main-thread only. It imports worker-safe search domain types
   and `quran-types` (type-only), so it pulls no $env /
   SvelteKit code and could be reused on either side of the worker boundary.
*/

import {
  isOpenerKind,
  isOpenerPackaging,
  isQuranScript,
  isQuranSourceId,
  OpenerKind,
  type Ayah,
  type ArtifactSpec,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";
import { SearchHitKind, type SearchHit } from "./search/types";
import { isCanonicalAyahCoordinate } from "./view/canonical-coordinates";
import { sourceProfile } from "./view/source-profiles";

function asRecord(raw: unknown): Record<string, unknown> | null {
  return raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
}

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

export function decodeSearchHit(raw: unknown): SearchHit | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  if (rec.kind === SearchHitKind.Opener) {
    const surah = positiveInteger(rec.surah, 114);
    const text = typeof rec.text === "string" ? rec.text : null;
    if (surah === null || text === null || rec.key !== `opener:${surah}`) return null;
    const highlights = decodeHighlights(rec.highlights, text.length);
    if (!highlights) return null;
    if (rec.anchorAyah !== 1) return null;
    return {
      kind: SearchHitKind.Opener,
      key: `opener:${surah}`,
      surah,
      anchorAyah: 1,
      text,
      highlights,
    };
  }
  if (rec.kind !== SearchHitKind.Ayah) return null;
  const ayah = decodeAyah(rec.ayah);
  if (!ayah) return null;
  const highlights = decodeHighlights(rec.highlights, ayah.text.length);
  if (!highlights) return null;
  return {
    kind: SearchHitKind.Ayah,
    ayah,
    highlights,
  };
}

function positiveInteger(value: unknown, max = Number.MAX_SAFE_INTEGER): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 1 && value <= max
    ? value
    : null;
}

function nonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function decodeAyah(raw: unknown): Ayah | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const surah = positiveInteger(rec.surah, 114);
  const ayah = positiveInteger(rec.ayah);
  const globalIndex = positiveInteger(rec.globalIndex, 6236);
  const text = typeof rec.text === "string" ? rec.text : null;
  if (
    surah === null ||
    ayah === null ||
    globalIndex === null ||
    text === null ||
    rec.key !== `${surah}:${ayah}` ||
    !isCanonicalAyahCoordinate(globalIndex, surah, ayah)
  ) {
    return null;
  }
  return { key: `${surah}:${ayah}`, surah, ayah, globalIndex, text };
}

function decodeHighlights(
  raw: unknown,
  textLength: number,
): { start: number; end: number }[] | null {
  if (!Array.isArray(raw)) return null;
  const out: { start: number; end: number }[] = [];
  for (const item of raw) {
    const rec = asRecord(item);
    if (!rec) return null;
    const start = nonNegativeInteger(rec.start);
    const end = nonNegativeInteger(rec.end);
    if (start === null || end === null || end <= start || end > textLength) return null;
    out.push({ start, end });
  }
  return out;
}

export function decodeSurahNormalization(raw: unknown): SurahNormalization | null {
  const rec = asRecord(raw);
  if (!rec || !isQuranSourceId(rec.sourceId) || !isQuranScript(rec.script)) return null;
  if (typeof rec.sourceProfile !== "string" || !isOpenerPackaging(rec.packaging)) {
    return null;
  }
  const profile = sourceProfile(rec.sourceId);
  if (profile.id !== rec.sourceProfile || profile.script !== rec.script) return null;
  if (!isOpenerKind(rec.openerKind)) return null;
  const openerEndScalar = nonNegativeInteger(rec.openerEndScalar);
  const bodyStartScalar = nonNegativeInteger(rec.bodyStartScalar);
  const surah = positiveInteger(rec.surah, 114);
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
 *  {@link unwrapEnvelope}. Returns null when any part of the response violates
 *  the tagged contract. This fail-closed behavior prevents an old ayah-only API
 *  from masquerading as a valid canonical response with an empty result list. */
export function decodeSearchResponse(raw: unknown): DecodedSearchPayload | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  if (!Array.isArray(rec.results)) return null;
  const results: SearchHit[] = [];
  for (const item of rec.results) {
    const hit = decodeSearchHit(item);
    if (!hit) return null;
    results.push(hit);
  }
  return {
    query: typeof rec.query === "string" ? rec.query : null,
    total: nonNegativeInteger(rec.total),
    limit: nonNegativeInteger(rec.limit),
    offset: nonNegativeInteger(rec.offset),
    results,
  };
}

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

export interface DecodedVersion {
  contentVersion: string | null;
  searchVersion: string | null;
}

export function decodeVersionPayload(rawBody: unknown): DecodedVersion | null {
  const data = asRecord(unwrapEnvelope(rawBody));
  if (!data) return null;
  return {
    contentVersion: typeof data.contentVersion === "string" ? data.contentVersion : null,
    searchVersion: typeof data.searchVersion === "string" ? data.searchVersion : null,
  };
}
