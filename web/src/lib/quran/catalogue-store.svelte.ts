import { QURAN } from "$lib/config/site";
import type { SourceCatalogueEntry, TranslationCatalogueEntry } from "$lib/data/quran-types";

import { bakedTranslationCatalogue, fetchSourceCatalogue, translationCatalogue } from "./catalogue";

const ERROR_BACKOFF_MS = 30_000;

type CatalogueStatus = "idle" | "loading" | "ready" | "degraded";

class SourceCatalogueStore {
  #baked = bakedTranslationCatalogue();
  #entries = $state.raw<SourceCatalogueEntry[]>([]);
  #status = $state<CatalogueStatus>("idle");
  #nextRetryAt = 0;
  #pending: Promise<SourceCatalogueEntry[]> | null = null;

  get entries(): SourceCatalogueEntry[] {
    return this.#status === "ready" ? this.#entries : this.#baked;
  }

  get translations(): TranslationCatalogueEntry[] {
    return translationCatalogue(this.entries);
  }

  get status(): CatalogueStatus {
    return this.#status;
  }

  ensure(): Promise<SourceCatalogueEntry[]> {
    if (this.#status === "ready") return Promise.resolve(this.#entries);
    if (this.#status === "degraded" && Date.now() < this.#nextRetryAt)
      return Promise.resolve(this.#entries);
    if (this.#pending) return this.#pending;
    this.#pending = this.#run();
    return this.#pending;
  }

  async #run(): Promise<SourceCatalogueEntry[]> {
    if (!QURAN.apiBase) {
      this.#entries = this.#baked;
      this.#status = "ready";
      this.#pending = null;
      return this.#entries;
    }
    this.#status = "loading";
    const live = await fetchSourceCatalogue();
    this.#pending = null;
    if (live.length > 0) {
      this.#entries = live;
      this.#status = "ready";
      return live;
    }
    this.#entries = this.#baked;
    this.#status = "degraded";
    this.#nextRetryAt = Date.now() + ERROR_BACKOFF_MS;
    return this.#entries;
  }
}

export const catalogueStore = new SourceCatalogueStore();
