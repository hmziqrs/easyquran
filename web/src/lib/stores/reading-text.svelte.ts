import { browser } from "$app/env";

import {
  asArray,
  asObject,
  asString,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

/**
 * What Reading mode flows: the Arabic, or one translation. Ayah-by-Ayah always shows the
 * Arabic plus every stacked translation; Reading shows exactly one text, and this store is
 * the reader's standing choice of which. It is a setting, never a route: switching it does
 * not navigate (quran.com's model), so there is no "primary translation" to manage.
 */
export type ReadingText = "arabic" | "translation";

const READING_TEXT_STORAGE_KEY = "easyquran.reader.reading-text";
const READING_TEXT_SCHEMA_VERSION = 1;
/** How many recent Reading picks the picker offers as one-tap chips. */
export const READING_RECENT_MAX = 8;

interface PersistedReadingText {
  v: number;
  text: ReadingText;
  translationId: string | null;
  recent: string[];
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); isFutureSchema/asObject/asString validate
function decodeReadingText(raw: unknown): PersistedReadingText {
  const fallback: PersistedReadingText = {
    v: READING_TEXT_SCHEMA_VERSION,
    text: "arabic",
    translationId: null,
    recent: [],
  };
  if (isFutureSchema(raw, READING_TEXT_SCHEMA_VERSION)) return fallback;
  const stored = asObject(raw);
  if (!stored) return fallback;
  return {
    v: READING_TEXT_SCHEMA_VERSION,
    text: asString(stored.text) === "translation" ? "translation" : "arabic",
    translationId: asString(stored.translationId) ?? null,
    recent: asArray(stored.recent, asString).slice(0, READING_RECENT_MAX),
  };
}

export class ReadingTextStore {
  #text = $state<ReadingText>("arabic");
  #translationId = $state<string | null>(null);
  #recent = $state<string[]>([]);
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    this.#load();
    this.#teardown = onStorageKey(READING_TEXT_STORAGE_KEY, () => this.#load());
  }

  #load(): void {
    const stored = decodeReadingText(readJSON(READING_TEXT_STORAGE_KEY));
    this.#text = stored.text;
    this.#translationId = stored.translationId;
    this.#recent = stored.recent;
  }

  #commit(): void {
    if (!browser) return;
    writeJSON(READING_TEXT_STORAGE_KEY, {
      v: READING_TEXT_SCHEMA_VERSION,
      text: this.#text,
      translationId: this.#translationId,
      recent: this.#recent,
    });
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  get text(): ReadingText {
    return this.#text;
  }

  /** The translation last chosen for Reading; null until the reader picks one. */
  get translationId(): string | null {
    return this.#translationId;
  }

  /** Translations picked for Reading, most recent first. */
  get recent(): readonly string[] {
    return this.#recent;
  }

  readArabic(): void {
    if (this.#text === "arabic") return;
    this.#text = "arabic";
    this.#commit();
  }

  /** Read a translation; `id` null keeps the last pick (or the first stacked one). */
  readTranslation(id: string | null = null): void {
    const nextId = id ?? this.#translationId;
    const nextRecent =
      nextId === null
        ? this.#recent
        : [nextId, ...this.#recent.filter((x) => x !== nextId)].slice(0, READING_RECENT_MAX);
    const recentSame =
      nextRecent.length === this.#recent.length &&
      nextRecent.every((x, i) => x === this.#recent[i]);
    if (this.#text === "translation" && nextId === this.#translationId && recentSame) return;
    this.#text = "translation";
    this.#translationId = nextId;
    this.#recent = nextRecent;
    this.#commit();
  }
}

export const readingText = new ReadingTextStore();
