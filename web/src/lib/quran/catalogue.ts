import { QURAN } from "$lib/config/site";
import {
  type SourceCatalogueEntry,
  type TranslationCatalogueEntry,
  type TranslationDirection,
} from "$lib/data/quran-types";
import rawTranslations from "../data/translations.json";
import { decodeSourcesPayload } from "./wire";

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
  return [
    id,
    language,
    languageCode,
    direction,
    name,
    translator,
    filePath,
    sizeBytes,
  ] as TranslationRow;
}

function bakedTranslationCatalogue(): SourceCatalogueEntry[] {
  return (rawTranslations as readonly unknown[]).map((raw, index) => {
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
    return { kind: "translation" as const, entry };
  });
}

function localizeDeliveryUrls(entries: readonly SourceCatalogueEntry[]): SourceCatalogueEntry[] {
  const translations = new Map(
    bakedTranslationCatalogue().map((item) => [
      item.kind === "translation" ? item.entry.id : "",
      item,
    ]),
  );
  const arabic = new Map(QURAN.scripts.map((spec) => [spec.id, spec]));
  return entries.map((item) => {
    if (item.kind === "translation") {
      const local = translations.get(item.entry.id);
      return local?.kind === "translation"
        ? { ...item, entry: { ...item.entry, downloadUrl: local.entry.downloadUrl } }
        : item;
    }
    const local = arabic.get(item.spec.id);
    return local ? { ...item, spec: { ...item.spec, downloadUrl: local.downloadUrl } } : item;
  });
}

const SOURCE_CATALOGUE_TTL_MS = 300_000;
let catalogueCache: { entries: SourceCatalogueEntry[]; expiresAt: number } | null = null;
let pendingCatalogue: Promise<SourceCatalogueEntry[]> | null = null;

async function fetchSourceCatalogue(): Promise<SourceCatalogueEntry[]> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), 3000);
  try {
    const res = await fetch(`${QURAN.apiBase}/sources`, {
      signal: ctrl.signal,
      headers: { accept: "application/json" },
    });
    if (!res.ok) return [];
    const entries = decodeSourcesPayload(await res.json());
    if (!entries) return [];
    return localizeDeliveryUrls(entries);
  } catch {
    return [];
  } finally {
    clearTimeout(timer);
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
