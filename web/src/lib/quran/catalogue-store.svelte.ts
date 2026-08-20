import type { TranslationCatalogueEntry } from "$lib/data/quran-types";

import { bakedTranslationCatalogue, translationCatalogue } from "./catalogue";

class SourceCatalogueStore {
  readonly #translations: TranslationCatalogueEntry[] = translationCatalogue(
    bakedTranslationCatalogue(),
  );

  get translations(): TranslationCatalogueEntry[] {
    return this.#translations;
  }
}

export const catalogueStore = new SourceCatalogueStore();
