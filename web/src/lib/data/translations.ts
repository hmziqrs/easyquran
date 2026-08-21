import type { TranslationDirection } from "./quran-types";
import rawTranslations from "./translations.json";

export const TRANSLATION_ARTIFACT_PREFIX = "tanzil/translations/";

const TranslationField = {
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

export interface BakedTranslationMetadata {
  readonly id: string;
  readonly language: string;
  readonly languageCode: string;
  readonly direction: TranslationDirection;
  readonly name: string;
  readonly translator: string | null;
  readonly filePath: string;
  readonly artifactPath: string;
  readonly sizeBytes: number;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- helper is part of sole raw JSON decoder below; it parses one field into a domain string
function nonEmptyString(value: unknown, field: string, row: number): string {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- validates loose JSON values at their sole decode boundary
  if (typeof value !== "string" || value === "") {
    throw new Error(`[translations] row ${row}: bad ${field}`);
  }
  return value;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- helper is part of sole raw JSON decoder below; it parses one field into TranslationDirection
function direction(value: unknown, row: number): TranslationDirection {
  if (value !== "rtl" && value !== "ltr") {
    throw new Error(`[translations] row ${row}: bad direction`);
  }
  return value;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw imported JSON is intentionally validated once here
function decodeTranslation(raw: unknown, index: number): BakedTranslationMetadata {
  const rowNumber = index + 1;
  if (!Array.isArray(raw) || raw.length !== TRANSLATION_FIELD_COUNT) {
    throw new Error(
      `[translations] invalid row ${rowNumber}: expected ${TRANSLATION_FIELD_COUNT} fields`,
    );
  }
  const translatorValue = raw[TranslationField.Translator];
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- nullable JSON field validated at decode boundary
  if (translatorValue !== null && typeof translatorValue !== "string") {
    throw new Error(`[translations] row ${rowNumber}: bad translator`);
  }
  const sizeBytes = raw[TranslationField.FileSize];
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- sole raw JSON boundary must establish number representation before integer validation
  if (typeof sizeBytes !== "number" || !Number.isSafeInteger(sizeBytes) || sizeBytes <= 0) {
    throw new Error(`[translations] row ${rowNumber}: bad sizeBytes`);
  }
  const filePath = nonEmptyString(raw[TranslationField.FilePath], "filePath", rowNumber);
  return Object.freeze({
    id: nonEmptyString(raw[TranslationField.Id], "id", rowNumber),
    language: nonEmptyString(raw[TranslationField.Language], "language", rowNumber),
    languageCode: nonEmptyString(
      raw[TranslationField.LanguageCode],
      "languageCode",
      rowNumber,
    ),
    direction: direction(raw[TranslationField.Direction], rowNumber),
    name: nonEmptyString(raw[TranslationField.Name], "name", rowNumber),
    translator: translatorValue,
    filePath,
    artifactPath: `${TRANSLATION_ARTIFACT_PREFIX}${filePath}`,
    sizeBytes,
  });
}

function uniqueIndex(
  entries: readonly BakedTranslationMetadata[],
  key: (entry: BakedTranslationMetadata) => string,
  label: string,
): ReadonlyMap<string, BakedTranslationMetadata> {
  const index = new Map<string, BakedTranslationMetadata>();
  for (const entry of entries) {
    const value = key(entry);
    if (index.has(value)) throw new Error(`[translations] duplicate ${label}: ${value}`);
    index.set(value, entry);
  }
  return index;
}

export const TRANSLATIONS: readonly BakedTranslationMetadata[] = Object.freeze(
  rawTranslations.map((raw, index) => decodeTranslation(raw, index)),
);

export const TRANSLATION_BY_ID: ReadonlyMap<string, BakedTranslationMetadata> = uniqueIndex(
  TRANSLATIONS,
  (entry) => entry.id,
  "id",
);

export const TRANSLATION_BY_ARTIFACT_PATH: ReadonlyMap<string, BakedTranslationMetadata> =
  uniqueIndex(
    TRANSLATIONS,
    (entry) => entry.artifactPath,
    "artifact path",
  );
