import type { QuranRangeText } from "$lib/data/quran-types";
import { QuranApiAvailability } from "$lib/quran/api-availability";

const DEFAULT_MAX_ENTRIES = 512;

export type TranslationRangeLoader = () => Promise<QuranRangeText>;

export class TranslationRangeCache {
  readonly #maxEntries: number;
  readonly #values = new Map<string, QuranRangeText>();
  readonly #inFlight = new Map<string, Promise<QuranRangeText>>();

  constructor(maxEntries = DEFAULT_MAX_ENTRIES) {
    if (!Number.isSafeInteger(maxEntries) || maxEntries <= 0) {
      throw new Error("translation range cache maxEntries must be a positive integer");
    }
    this.#maxEntries = maxEntries;
  }

  get size(): number {
    return this.#values.size;
  }

  getOrLoad(key: string, load: TranslationRangeLoader): Promise<QuranRangeText> {
    const cached = this.#values.get(key);
    if (cached) {
      this.#values.delete(key);
      this.#values.set(key, cached);
      return Promise.resolve(cached);
    }

    const pending = this.#inFlight.get(key);
    if (pending) return pending;

    const request = load().then((value) => {
      this.#values.set(key, value);
      this.#prune();
      return value;
    });
    this.#inFlight.set(key, request);
    const clearPending = (): void => {
      if (this.#inFlight.get(key) === request) this.#inFlight.delete(key);
    };
    void request.then(clearPending, clearPending);
    return request;
  }

  clear(): void {
    this.#values.clear();
    this.#inFlight.clear();
  }

  #prune(): void {
    while (this.#values.size > this.#maxEntries) {
      const oldest = this.#values.keys().next().value;
      if (oldest === undefined) return;
      this.#values.delete(oldest);
    }
  }
}

const translationRangeCache = new TranslationRangeCache();
const serverQuranApiAvailability = new QuranApiAvailability();

export function translationRangeCacheKey(sourceId: string, from: number, to: number): string {
  return `${sourceId}:${from}-${to}`;
}

export function getCachedTranslationRange(
  sourceId: string,
  from: number,
  to: number,
  load: TranslationRangeLoader,
): Promise<QuranRangeText> {
  return translationRangeCache.getOrLoad(translationRangeCacheKey(sourceId, from, to), () =>
    serverQuranApiAvailability.run(load),
  );
}

export function clearTranslationRangeCache(): void {
  translationRangeCache.clear();
  serverQuranApiAvailability.reset();
}
