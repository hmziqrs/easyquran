import {
  isOpenerKind,
  isOpenerPackaging,
  isQuranScript,
  isQuranSourceId,
  OpenerKind,
  OpenerPackaging,
  QuranScript,
  SourceKind,
  type Ayah,
  type ArtifactSpec,
  type DownloadableSpec,
  type QuranRangeText,
  type QuranSurahText,
  type SourceCatalogueEntry,
  type SurahNormalization,
  type TranslationCatalogueEntry,
} from "$lib/data/quran-types";
import { isNumber, isString } from "es-toolkit";

import { SearchHitKind, type SearchHit } from "./search/types";
import { sourceProfile } from "./view/source-profiles";

export type AyahCoordinateValidator = (globalIndex: number, surah: number, ayah: number) => boolean;

/** First candidate that is actually an array. Payload envelopes spell the same list several ways. */
function firstArray(...candidates: unknown[]): unknown[] | null {
  for (const candidate of candidates) {
    if (Array.isArray(candidate)) return candidate;
  }
  return null;
}

// eslint-disable-next-line anti-slop/no-unsafe-dictionary-type -- decoded JSON object bag: every field read below is followed by a per-field decode check
type WireRecord = Record<string, unknown>;

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire-boundary parser: raw is arbitrary fetch/Worker JSON; this fn establishes the record contract
function asRecord(raw: unknown): WireRecord | null {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- boundary discrimination of JSON objects from primitives; arrays pass through and fail later field checks
  if (!raw || typeof raw !== "object") return null;
  // SAFETY: truthy + typeof "object" excludes null, undefined, and primitives; JSON never yields functions, so raw is a string-keyed record.
  return raw as WireRecord;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire-boundary unwrap: raw is arbitrary fetch/Worker JSON
export function unwrapEnvelope(raw: unknown) {
  const rec = asRecord(raw);
  if (!rec) return raw;
  return rec.data ?? raw;
}

export function decodeSearchHit(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified search API JSON; this fn is the parser
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): SearchHit | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  if (rec.kind === SearchHitKind.Opener) {
    const surah = positiveInteger(rec.surah, 114);
    const text = isString(rec.text) ? rec.text : null;
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

// eslint-disable-next-line anti-slop/no-unknown-parameters -- field decoder: value is an unvalidated JSON field; the number check below is the parse
function positiveInteger(value: unknown, max = Number.MAX_SAFE_INTEGER): number | null {
  return isNumber(value) && Number.isSafeInteger(value) && value >= 1 && value <= max
    ? value
    : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- field decoder: value is an unvalidated JSON field; the number check below is the parse
function nonNegativeInteger(value: unknown): number | null {
  return isNumber(value) && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
function decodeAyah(raw: unknown, validateCoordinate?: AyahCoordinateValidator): Ayah | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const surah = positiveInteger(rec.surah, 114);
  const ayah = positiveInteger(rec.ayah);
  const globalIndex = positiveInteger(rec.globalIndex, 6236);
  const text = isString(rec.text) ? rec.text : null;
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

function decodeRangeText(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
  raw: unknown,
  validateCoordinate: AyahCoordinateValidator | undefined,
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- decodeNorm consumes an unvalidated normalizations[] JSON element
  decodeNorm: (raw: unknown) => SurahNormalization | null,
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
    const normalization = decodeNorm(item);
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

export function decodeQuranRangeText(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): QuranRangeText | null {
  return decodeRangeText(raw, validateCoordinate, decodeSurahNormalization);
}

function decodeHighlights(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- field decoder: raw is the unvalidated highlights JSON array
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

/**
 * `null` = legitimately absent, `undefined` = malformed for this opener kind, which makes the
 * whole record invalid. An opener-less Surah must carry an explicitly null openerText.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- field decoder: value is the unvalidated openerText JSON field
function decodeOpenerText(openerKind: OpenerKind, value: unknown): string | null | undefined {
  if (openerKind === OpenerKind.None) return value === null ? null : undefined;
  return isString(value) ? value : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
function decodeSurahNormalization(raw: unknown): SurahNormalization | null {
  const rec = asRecord(raw);
  if (
    !rec ||
    !isString(rec.sourceId) ||
    !isQuranSourceId(rec.sourceId) ||
    !isString(rec.script) ||
    !isQuranScript(rec.script)
  ) {
    return null;
  }
  if (
    !isString(rec.sourceProfile) ||
    !isString(rec.packaging) ||
    !isOpenerPackaging(rec.packaging)
  ) {
    return null;
  }
  const profile = sourceProfile(rec.sourceId);
  if (profile.id !== rec.sourceProfile || profile.script !== rec.script) return null;
  if (!isString(rec.openerKind) || !isOpenerKind(rec.openerKind)) return null;
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
  const openerText = decodeOpenerText(rec.openerKind, rec.openerText);
  if (openerText === undefined) return null;
  return {
    surah,
    sourceId: rec.sourceId,
    script: rec.script,
    sourceProfile: rec.sourceProfile,
    packaging: rec.packaging,
    openerKind: rec.openerKind,
    openerText,
    openerEndScalar,
    bodyStartScalar,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
export function decodeQuranSurahText(raw: unknown): QuranSurahText | null {
  const rec = asRecord(raw);
  if (
    !rec ||
    !isString(rec.sourceId) ||
    !isQuranSourceId(rec.sourceId) ||
    !isString(rec.script) ||
    !isQuranScript(rec.script)
  ) {
    return null;
  }
  if (
    !Array.isArray(rec.verses) ||
    !rec.verses.every((verse): verse is string => isString(verse))
  ) {
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
    verses: rec.verses.slice(),
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
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified search API JSON; this fn is the parser
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
    query: isString(rec.query) ? rec.query : null,
    total: nonNegativeInteger(rec.total),
    limit: nonNegativeInteger(rec.limit),
    offset: nonNegativeInteger(rec.offset),
    results,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified manifest JSON; this fn is the parser
export function decodeScript(raw: unknown): ArtifactSpec | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const id = rec.id;
  if (!isString(id) || !isQuranSourceId(id)) return null;
  const sizeBytes = positiveInteger(rec.sizeBytes);
  const downloadUrl = rec.downloadUrl;
  if (sizeBytes === null || !isString(downloadUrl) || downloadUrl.length === 0) {
    return null;
  }
  return { id, sizeBytes, downloadUrl };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: rawBody is unverified manifest JSON; this fn is the parser
export function decodeScriptsPayload(rawBody: unknown): ArtifactSpec[] | null {
  const data = asRecord(unwrapEnvelope(rawBody));
  if (!data) return null;
  if (!Array.isArray(data.scripts)) return null;
  const out: ArtifactSpec[] = [];
  for (const item of data.scripts) {
    const spec = decodeScript(item);
    if (!spec) return null;
    out.push(spec);
  }
  return out;
}

function decodeTranslationCatalogueEntry(rec: WireRecord): TranslationCatalogueEntry | null {
  const id = isString(rec.id) && rec.id.length > 0 ? rec.id : null;
  const language = isString(rec.language) && rec.language.length > 0 ? rec.language : null;
  const languageCode =
    isString(rec.languageCode) && rec.languageCode.length > 0 ? rec.languageCode : null;
  const direction = rec.direction === "rtl" || rec.direction === "ltr" ? rec.direction : null;
  const name = isString(rec.name) && rec.name.length > 0 ? rec.name : null;
  const translator =
    rec.translator === null || isString(rec.translator) ? rec.translator : undefined;
  const sizeBytes = positiveInteger(rec.sizeBytes);
  const downloadUrl =
    isString(rec.downloadUrl) && rec.downloadUrl.length > 0 ? rec.downloadUrl : null;
  if (
    !id ||
    !language ||
    !languageCode ||
    !direction ||
    !name ||
    translator === undefined ||
    sizeBytes === null ||
    !downloadUrl
  ) {
    return null;
  }
  return {
    id,
    language,
    languageCode,
    direction,
    name,
    translator,
    sizeBytes,
    downloadUrl,
  };
}

function decodeArabicCatalogueSpec(rec: WireRecord): ArtifactSpec | null {
  const id = rec.id;
  if (!isString(id) || !isQuranSourceId(id)) return null;
  const sizeBytes = positiveInteger(rec.sizeBytes);
  const downloadUrl =
    isString(rec.downloadUrl) && rec.downloadUrl.length > 0 ? rec.downloadUrl : null;
  if (sizeBytes === null || downloadUrl === null) return null;
  return { id, sizeBytes, downloadUrl };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified manifest JSON; this fn is the parser
function decodeSourceCatalogueEntry(raw: unknown): SourceCatalogueEntry | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const kind = isString(rec.kind) ? rec.kind : null;
  if (kind === SourceKind.Translation) {
    const entry = decodeTranslationCatalogueEntry(rec);
    return entry ? { kind: SourceKind.Translation, entry } : null;
  }
  if (kind === SourceKind.Arabic) {
    const spec = decodeArabicCatalogueSpec(rec);
    return spec ? { kind: SourceKind.Arabic, spec } : null;
  }
  const spec = decodeArabicCatalogueSpec(rec);
  if (spec) return { kind: SourceKind.Arabic, spec };
  const entry = decodeTranslationCatalogueEntry(rec);
  return entry ? { kind: SourceKind.Translation, entry } : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: rawBody is unverified manifest JSON; this fn is the parser
export function decodeSourcesPayload(rawBody: unknown): SourceCatalogueEntry[] | null {
  const data = asRecord(unwrapEnvelope(rawBody));
  if (!data) return null;
  const list = firstArray(data.scripts, data.sources);
  if (!list) return null;
  const out: SourceCatalogueEntry[] = [];
  for (const item of list) {
    const entry = decodeSourceCatalogueEntry(item);
    if (!entry) return null;
    out.push(entry);
  }
  return out;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
function decodeTranslationNormalization(raw: unknown): SurahNormalization | null {
  const rec = asRecord(raw);
  if (!rec || rec.script !== QuranScript.Translation) return null;
  if (!isString(rec.sourceId) || rec.sourceId.length === 0) return null;
  if (!isString(rec.sourceProfile)) return null;
  if (rec.packaging !== OpenerPackaging.Absent) return null;
  if (rec.openerKind !== OpenerKind.None) return null;
  if (rec.openerText !== null) return null;
  const surah = positiveInteger(rec.surah, 114);
  const openerEndScalar = nonNegativeInteger(rec.openerEndScalar);
  const bodyStartScalar = nonNegativeInteger(rec.bodyStartScalar);
  if (surah === null || openerEndScalar !== 0 || bodyStartScalar !== 0) return null;
  return {
    surah,
    sourceId: rec.sourceId,
    script: QuranScript.Translation,
    sourceProfile: rec.sourceProfile,
    packaging: OpenerPackaging.Absent,
    openerKind: OpenerKind.None,
    openerText: null,
    openerEndScalar: 0,
    bodyStartScalar: 0,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
export function decodeTranslationSurahText(raw: unknown): QuranSurahText | null {
  const rec = asRecord(raw);
  if (!rec) return null;
  const sourceId = isString(rec.sourceId) && rec.sourceId.length > 0 ? rec.sourceId : null;
  if (!sourceId || rec.script !== QuranScript.Translation) return null;
  if (
    !Array.isArray(rec.verses) ||
    !rec.verses.every((verse): verse is string => isString(verse))
  ) {
    return null;
  }
  const normalization = decodeTranslationNormalization(rec.normalization);
  if (!normalization || normalization.sourceId !== sourceId) return null;
  return {
    sourceId,
    script: QuranScript.Translation,
    verses: rec.verses.slice(),
    normalization,
  };
}

export function decodeTranslationRangeText(
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is unverified Worker JSON; this fn is the parser
  raw: unknown,
  validateCoordinate?: AyahCoordinateValidator,
): QuranRangeText | null {
  return decodeRangeText(raw, validateCoordinate, decodeTranslationNormalization);
}

export interface BakedArtifactEntry {
  readonly sizeBytes: number;
  readonly r2Path: string;
  readonly sameOriginDeliveryPath: string;
}

export type BakedArtifactMap = ReadonlyMap<string, BakedArtifactEntry>;

export interface ValidatedArtifact {
  readonly id: string;
  readonly sizeBytes: number;
  readonly downloadUrl: string;
}

export function buildArabicBakedMap(
  scripts: readonly DownloadableSpec[],
  artifactBase: string,
): BakedArtifactMap {
  const map = new Map<string, BakedArtifactEntry>();
  for (const spec of scripts) {
    const r2Path = spec.downloadUrl.startsWith(artifactBase)
      ? spec.downloadUrl.slice(artifactBase.length)
      : "";
    map.set(spec.id, {
      sizeBytes: spec.sizeBytes,
      r2Path,
      sameOriginDeliveryPath: spec.downloadUrl,
    });
  }
  return map;
}

export function reportArtifactRejection(reason: string): void {
  void import("$lib/stores/consent.svelte")
    .then((mod) =>
      mod.consent.analytics
        ? import("$lib/firebase/analytics").then((m) =>
            m.track("quran_artifact_rejected", { reason }),
          )
        : null,
    )
    .catch(() => {});
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- wire decoder boundary: raw is an unvalidated manifest downloadUrl field
export function parseArtifactUrl(raw: unknown, baked: BakedArtifactEntry): URL | null {
  if (!isString(raw) || raw.length === 0) return null;
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }
  if (url.protocol !== "https:") return null;
  if (url.username !== "" || url.password !== "") return null;
  if (url.search !== "" || url.hash !== "") return null;
  if (url.pathname !== baked.r2Path) return null;
  return url;
}

export function validateArtifactAgainstBaked(
  id: string,
  sizeBytes: number,
  downloadUrl: string,
  baked: BakedArtifactMap,
): ValidatedArtifact | null {
  const entry = baked.get(id);
  if (!entry) return null;
  if (sizeBytes !== entry.sizeBytes) return null;
  if (!parseArtifactUrl(downloadUrl, entry)) return null;
  return {
    id,
    sizeBytes: entry.sizeBytes,
    downloadUrl: entry.sameOriginDeliveryPath,
  };
}
