import { QURAN } from "$lib/config/site";
import type { TranslationCatalogueEntry } from "$lib/data/quran-types";
import { TRANSLATIONS } from "$lib/data/translations";

export const TRANSLATION_CATALOGUE: readonly TranslationCatalogueEntry[] = Object.freeze(
  TRANSLATIONS.map((translation) =>
    Object.freeze({
      id: translation.id,
      language: translation.language,
      languageCode: translation.languageCode,
      direction: translation.direction,
      name: translation.name,
      translator: translation.translator,
      sizeBytes: translation.sizeBytes,
      downloadUrl: `${QURAN.artifactBase}/${translation.artifactPath}`,
    }),
  ),
);

export const TRANSLATION_CATALOGUE_BY_ID: ReadonlyMap<string, TranslationCatalogueEntry> = new Map(
  TRANSLATION_CATALOGUE.map((entry) => [entry.id, entry]),
);

export function peekTranslationName(sourceId: string | undefined): string | null {
  if (!sourceId) return null;
  return TRANSLATION_CATALOGUE_BY_ID.get(sourceId)?.name ?? null;
}
