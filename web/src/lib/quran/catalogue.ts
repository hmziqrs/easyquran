import { QURAN } from "$lib/config/site";
import {
  type SourceCatalogueEntry,
  type TranslationCatalogueEntry,
  type TranslationDirection,
} from "$lib/data/quran-types";
import { fetchWithTimeout } from "$lib/quran/fetch";

import rawTranslations from "../data/translations.json";
import {
  type BakedArtifactEntry,
  type BakedArtifactMap,
  buildArabicBakedMap,
  decodeSourcesPayload,
  reportArtifactRejection,
  validateArtifactAgainstBaked,
} from "./wire";

export const TranslationField = {
  Id: 0,
  Language: 1,
  LanguageCode: 2,
  Direction: 3,
  Name: 4,
  Translator: 5,
  FilePath: 6,
  FileSize: 7,
} as const;

const TRANSLATION_FIELD_COUNT = 8;

type TranslationRow = readonly [
  id: string,
  language: string,
  languageCode: string,
  direction: TranslationDirection,
  name: string,
  translator: string | null,
  filePath: string,
  sizeBytes: number,
];

function isTranslationDirection(value: unknown): value is TranslationDirection {
  return value === "rtl" || value === "ltr";
}

function decodeTranslationRow(raw: unknown, index: number): TranslationRow {
  if (!Array.isArray(raw) || raw.length !== TRANSLATION_FIELD_COUNT) {
    throw new Error(
      `[catalogue] invalid translation row ${index + 1}: expected ${TRANSLATION_FIELD_COUNT} fields`,
    );
  }
  const id = raw[TranslationField.Id];
  const language = raw[TranslationField.Language];
  const languageCode = raw[TranslationField.LanguageCode];
  const direction = raw[TranslationField.Direction];
  const name = raw[TranslationField.Name];
  const translator = raw[TranslationField.Translator];
  const filePath = raw[TranslationField.FilePath];
  const sizeBytes = raw[TranslationField.FileSize];
  if (typeof id !== "string" || !id) throw new Error(`[catalogue] row ${index + 1}: bad id`);
  if (typeof language !== "string" || !language)
    throw new Error(`[catalogue] row ${index + 1}: bad language`);
  if (typeof languageCode !== "string" || !languageCode)
    throw new Error(`[catalogue] row ${index + 1}: bad languageCode`);
  if (!isTranslationDirection(direction))
    throw new Error(`[catalogue] row ${index + 1}: bad direction`);
  if (typeof name !== "string" || !name) throw new Error(`[catalogue] row ${index + 1}: bad name`);
  if (translator !== null && typeof translator !== "string")
    throw new Error(`[catalogue] row ${index + 1}: bad translator`);
  if (typeof filePath !== "string" || !filePath)
    throw new Error(`[catalogue] row ${index + 1}: bad filePath`);
  if (!Number.isSafeInteger(sizeBytes) || sizeBytes <= 0)
    throw new Error(`[catalogue] row ${index + 1}: bad sizeBytes`);
  const row: TranslationRow = [
    id,
    language,
    languageCode,
    direction,
    name,
    translator,
    filePath,
    sizeBytes,
  ];
  return row;
}

interface DecodedTranslation {
  row: TranslationRow;
  entry: TranslationCatalogueEntry;
}

function decodeBakedTranslations(): DecodedTranslation[] {
  return rawTranslations.map((raw, index) => {
    const row = decodeTranslationRow(raw, index);
    const entry: TranslationCatalogueEntry = {
      id: row[TranslationField.Id],
      language: row[TranslationField.Language],
      languageCode: row[TranslationField.LanguageCode],
      direction: row[TranslationField.Direction],
      name: row[TranslationField.Name],
      translator: row[TranslationField.Translator],
      sizeBytes: row[TranslationField.FileSize],
      downloadUrl: `${QURAN.artifactBase}/tanzil/translations/${row[TranslationField.FilePath]}`,
    };
    return { row, entry };
  });
}

export function bakedTranslationCatalogue(): SourceCatalogueEntry[] {
  return decodeBakedTranslations().map(({ entry }) => ({
    kind: "translation" as const,
    entry,
  }));
}

function bakedTranslationArtifactMap(): BakedArtifactMap {
  const map = new Map<string, BakedArtifactEntry>();
  for (const { row, entry } of decodeBakedTranslations()) {
    map.set(entry.id, {
      sizeBytes: entry.sizeBytes,
      r2Path: `/tanzil/translations/${row[TranslationField.FilePath]}`,
      sameOriginDeliveryPath: entry.downloadUrl,
    });
  }
  return map;
}

function validateAndLocalizeCatalogue(
  entries: readonly SourceCatalogueEntry[],
): SourceCatalogueEntry[] | null {
  const translations = bakedTranslationArtifactMap();
  const arabic = buildArabicBakedMap(QURAN.scripts, QURAN.artifactBase);
  const out: SourceCatalogueEntry[] = [];
  for (const item of entries) {
    if (item.kind === "translation") {
      const validated = validateArtifactAgainstBaked(
        item.entry.id,
        item.entry.sizeBytes,
        item.entry.downloadUrl,
        translations,
      );
      if (!validated) return null;
      out.push({
        kind: "translation",
        entry: {
          ...item.entry,
          sizeBytes: validated.sizeBytes,
          downloadUrl: validated.downloadUrl,
        },
      });
    } else {
      const validated = validateArtifactAgainstBaked(
        item.spec.id,
        item.spec.sizeBytes,
        item.spec.downloadUrl,
        arabic,
      );
      if (!validated) return null;
      out.push({
        kind: "arabic",
        spec: {
          ...item.spec,
          sizeBytes: validated.sizeBytes,
          downloadUrl: validated.downloadUrl,
        },
      });
    }
  }
  return out;
}

const SOURCE_CATALOGUE_TTL_MS = 300_000;
let catalogueCache: { entries: SourceCatalogueEntry[]; expiresAt: number } | null = null;
let pendingCatalogue: Promise<SourceCatalogueEntry[]> | null = null;

export async function fetchSourceCatalogue(signal?: AbortSignal): Promise<SourceCatalogueEntry[]> {
  try {
    const res = await fetchWithTimeout(`${QURAN.apiBase}/sources`, {
      signal,
      headers: { accept: "application/json" },
    });
    if (!res.ok) return [];
    const entries = decodeSourcesPayload(await res.json());
    if (!entries) {
      reportArtifactRejection("sources_payload_malformed");
      return [];
    }
    const validated = validateAndLocalizeCatalogue(entries);
    if (!validated) {
      reportArtifactRejection("sources_payload");
      return [];
    }
    return validated;
  } catch {
    return [];
  }
}

export async function resolveSourceCatalogue(
  signal?: AbortSignal,
): Promise<SourceCatalogueEntry[]> {
  if (!QURAN.apiBase) return bakedTranslationCatalogue();
  const cached = catalogueCache;
  if (cached && cached.expiresAt > Date.now()) {
    return cached.entries;
  }
  if (signal?.aborted) return [];
  if (!pendingCatalogue) {
    pendingCatalogue = fetchSourceCatalogue()
      .then((entries) => {
        if (entries.length > 0) {
          catalogueCache = { entries, expiresAt: Date.now() + SOURCE_CATALOGUE_TTL_MS };
          return entries;
        }
        return bakedTranslationCatalogue();
      })
      .finally(() => {
        pendingCatalogue = null;
      });
  }
  return pendingCatalogue;
}

export function translationCatalogue(
  entries: readonly SourceCatalogueEntry[],
): TranslationCatalogueEntry[] {
  const out: TranslationCatalogueEntry[] = [];
  for (const entry of entries) {
    if (entry.kind === "translation") out.push(entry.entry);
  }
  return out;
}

export function findCatalogueEntry(
  entries: readonly SourceCatalogueEntry[],
  sourceId: string,
): SourceCatalogueEntry | undefined {
  return entries.find((entry) =>
    entry.kind === "translation" ? entry.entry.id === sourceId : entry.spec.id === sourceId,
  );
}

export function peekTranslationName(sourceId: string | undefined): string | null {
  if (!sourceId) return null;
  const entry = findCatalogueEntry(bakedTranslationCatalogue(), sourceId);
  return entry?.kind === "translation" ? entry.entry.name : null;
}
