import { QURAN } from "$lib/config/site";
import {
  type SourceCatalogueEntry,
  type TranslationCatalogueEntry,
  type TranslationDirection,
} from "$lib/data/quran-types";

import rawTranslations from "../data/translations.json";

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

function isTranslationDirection(value: string | number): value is TranslationDirection {
  return value === "rtl" || value === "ltr";
}

function isNonEmptyString(value: string | number | undefined): value is string {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- discriminating a JSON import field that TS only types as a loose union; no schema seam exists for baked JSON
  return typeof value === "string" && value !== "";
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- the JSON import only types rows as a loose (string|number)[] union; per-field roles are established by the guards below
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
  if (!isNonEmptyString(id)) throw new Error(`[catalogue] row ${index + 1}: bad id`);
  if (!isNonEmptyString(language))
    throw new Error(`[catalogue] row ${index + 1}: bad language`);
  if (!isNonEmptyString(languageCode))
    throw new Error(`[catalogue] row ${index + 1}: bad languageCode`);
  if (!isTranslationDirection(direction))
    throw new Error(`[catalogue] row ${index + 1}: bad direction`);
  if (!isNonEmptyString(name)) throw new Error(`[catalogue] row ${index + 1}: bad name`);
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- translator is nullable in the row union; typeof verifies the wire field at the JSON boundary
  if (translator !== null && typeof translator !== "string")
    throw new Error(`[catalogue] row ${index + 1}: bad translator`);
  if (!isNonEmptyString(filePath))
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
