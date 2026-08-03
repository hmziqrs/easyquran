import {
  isOpenerKind,
  isOpenerPackaging,
  isQuranScript,
  isQuranSourceId,
  OpenerKind,
  type Ayah,
  type ArtifactSpec,
  type QuranRangeText,
  type QuranSurahText,
  type SurahNormalization,
} from "$lib/data/quran-types";
import { SearchHitKind, type SearchHit } from "./search/types";
import { sourceProfile } from "./view/source-profiles";

export type AyahCoordinateValidator = (globalIndex: number, surah: number, ayah: number) => boolean;

function asRecord(raw: unknown): Record<string, unknown> | null {
  return raw && typeof raw === "object" ? (raw as Record<string, unknown>) : null;
}

export function unwrapEnvelope(raw: unknown): unknown {
  const rec = asRecord(raw);
  if (!rec) return raw;
  return rec.data ?? raw;
}

export function decodeSearchHit(
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): SearchHit | null {
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
  const ayah = decodeAyah(rec.ayah, validateCoordinate);
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

function decodeAyah(raw: unknown, validateCoordinate?: AyahCoordinateValidator): Ayah | null {
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
    (validateCoordinate && !validateCoordinate(globalIndex, surah, ayah))
  ) {
    return null;
  }
  return { key: `${surah}:${ayah}`, surah, ayah, globalIndex, text };
}

export function decodeQuranRangeText(
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): QuranRangeText | null {
  const rec = asRecord(raw);
  if (!rec || !Array.isArray(rec.ayahs) || !Array.isArray(rec.normalizations)) return null;
  const ayahs: Ayah[] = [];
  for (const item of rec.ayahs) {
    const ayah = decodeAyah(item, validateCoordinate);
    if (!ayah) return null;
    const previous = ayahs.at(-1);
    if (previous && ayah.globalIndex !== previous.globalIndex + 1) return null;
    ayahs.push(ayah);
  }
  const normalizations: SurahNormalization[] = [];
  for (const item of rec.normalizations) {
    const normalization = decodeSurahNormalization(item);
    if (!normalization) return null;
    normalizations.push(normalization);
  }
  const represented = new Set(ayahs.map((ayah) => ayah.surah));
  if (
    represented.size !== normalizations.length ||
    normalizations.some((normalization) => !represented.has(normalization.surah))
  ) {
    return null;
  }
  return { ayahs, normalizations };
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

export function decodeSearchResponse(
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): DecodedSearchPayload | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  if (!Array.isArray(rec.results)) return null;
  const results: SearchHit[] = [];
  for (const item of rec.results) {
    const hit = decodeSearchHit(item, validateCoordinate);
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
  const sizeBytes = positiveInteger(rec.sizeBytes);
  const sha256 = rec.sha256;
  const downloadUrl = rec.downloadUrl;
  if (
    sizeBytes === null ||
    typeof sha256 !== "string" ||
    sha256.length === 0 ||
    typeof downloadUrl !== "string" ||
    downloadUrl.length === 0
  ) {
    return null;
  }
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
