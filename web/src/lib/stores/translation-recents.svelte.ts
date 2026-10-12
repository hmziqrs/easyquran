import { browser } from "$app/env";

import {
  asArray,
  asNumber,
  asObject,
  asString,
  asStringRecord,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

const TRANSLATION_RECENTS_STORAGE_KEY = "easyquran.translations.recent";
const TRANSLATION_RECENTS_SCHEMA_VERSION = 1;
/** How many recently used translations the MRU keeps. */
export const TRANSLATION_RECENTS_MAX = 20;

export interface TranslationRecentEntry {
  readonly id: string;
  readonly lastUsedAt: number;
  readonly useCount: number;
}

interface PersistedTranslationRecents {
  v: number;
  entries: TranslationRecentEntry[];
  /**
   * Feature 1.4: author key → last-used variant id. The MRU caps at 20 and
   * evicts; this map keeps each author's last-used variant alive regardless.
   */
  authors: Record<string, string>;
}

/** Write-side envelope: `authors` is added only once a variant memory exists. */
interface RecentsEnvelope {
  v: number;
  entries: TranslationRecentEntry[];
  authors?: Record<string, string>;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asString/asNumber validate
function decodeEntry(raw: unknown): TranslationRecentEntry | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  const id = asString(stored.id);
  const lastUsedAt = asNumber(stored.lastUsedAt, 0, Number.MAX_SAFE_INTEGER);
  const useCount = asNumber(stored.useCount, 1, Number.MAX_SAFE_INTEGER);
  if (id === undefined || id === "" || lastUsedAt === undefined || useCount === undefined) {
    return undefined;
  }
  return { id, lastUsedAt, useCount };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); isFutureSchema/asObject/asArray validate
function decodeTranslationRecents(raw: unknown): PersistedTranslationRecents {
  const fallback: PersistedTranslationRecents = {
    v: TRANSLATION_RECENTS_SCHEMA_VERSION,
    entries: [],
    authors: {},
  };
  if (isFutureSchema(raw, TRANSLATION_RECENTS_SCHEMA_VERSION)) return fallback;
  const stored = asObject(raw);
  if (!stored) return fallback;
  // Stored order IS the MRU order; decode drops duplicates and malformed entries.
  const seen = new Set<string>();
  const entries: TranslationRecentEntry[] = [];
  for (const entry of asArray(stored.entries, decodeEntry)) {
    if (seen.has(entry.id)) continue;
    seen.add(entry.id);
    entries.push(entry);
    if (entries.length >= TRANSLATION_RECENTS_MAX) break;
  }
  const authors: Record<string, string> = {};
  for (const [key, id] of Object.entries(asStringRecord(stored.authors))) {
    if (key !== "" && id !== "") authors[key] = id;
  }
  return { v: TRANSLATION_RECENTS_SCHEMA_VERSION, entries, authors };
}

/**
 * Translations the reader explicitly chose, most recently used first. Every
 * explicit selection records here — picker rows, one-tap chips, search picks —
 * while landing on a /t/… URL records nothing (a shared link is not a choice).
 * The picker's Recent group, its rail boost, and Reading's chip order all rank
 * from this MRU. Persisted to localStorage, synced across tabs.
 */
export class TranslationRecentsStore {
  #entries = $state<TranslationRecentEntry[]>([]);
  #authorVariants = $state.raw<Record<string, string>>({});
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    this.#load();
    this.#teardown = onStorageKey(TRANSLATION_RECENTS_STORAGE_KEY, () => this.#load());
  }

  #load(): void {
    const stored = decodeTranslationRecents(readJSON(TRANSLATION_RECENTS_STORAGE_KEY));
    this.#entries = stored.entries;
    this.#authorVariants = stored.authors;
  }

  #commit(entries: TranslationRecentEntry[], authors: Record<string, string>): void {
    this.#entries = entries;
    this.#authorVariants = authors;
    if (browser) {
      // `authors` rides only once a variant memory exists, so envelopes written
      // by author-less recorders stay byte-identical to the v1 shape.
      const envelope: RecentsEnvelope = { v: TRANSLATION_RECENTS_SCHEMA_VERSION, entries };
      if (Object.keys(authors).length > 0) envelope.authors = authors;
      writeJSON(TRANSLATION_RECENTS_STORAGE_KEY, envelope);
    }
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  /** Translation ids, most recently used first. */
  get ids(): readonly string[] {
    return this.#entries.map((entry) => entry.id);
  }

  /** The MRU rank of `id` (0 = most recent), or -1 when it was never used. */
  rankOf(id: string): number {
    return this.#entries.findIndex((entry) => entry.id === id);
  }

  /**
   * Reorders `ids` most recently used first. Never-used ids keep their given
   * order after every ranked one (stable sort), so callers can delegate only
   * the ordering and keep their own persisted list.
   */
  rankIds(ids: readonly string[]): string[] {
    return [...ids].sort((a, b) => this.#unrankedAfter(a) - this.#unrankedAfter(b));
  }

  /**
   * Records one explicit selection: bumps `id` to the front and counts the use.
   * `authorKey` (Feature 1.4, from translationAuthorKey) also notes the
   * author's last-used variant, which survives the MRU's 20-entry cap.
   */
  record(id: string, at: number = Date.now(), authorKey?: string): void {
    if (id === "") return;
    const previous = this.#entries.find((entry) => entry.id === id);
    const entry: TranslationRecentEntry = {
      id,
      lastUsedAt: at,
      useCount: previous === undefined ? 1 : previous.useCount + 1,
    };
    const authors = { ...this.#authorVariants };
    if (authorKey !== undefined && authorKey !== "") authors[authorKey] = id;
    this.#commit(
      [entry, ...this.#entries.filter((existing) => existing.id !== id)].slice(
        0,
        TRANSLATION_RECENTS_MAX,
      ),
      authors,
    );
  }

  /** The author's last-used variant id (Feature 1.4), or undefined when unknown. */
  authorVariant(authorKey: string): string | undefined {
    return this.#authorVariants[authorKey];
  }

  // Sort key for rankIds: never-used ids share one rank after every used one.
  #unrankedAfter(id: string): number {
    const rank = this.rankOf(id);
    return rank === -1 ? Number.MAX_SAFE_INTEGER : rank;
  }
}

export const translationRecents = new TranslationRecentsStore();
