import { browser } from "$app/env";

import {
  asArray,
  asNumber,
  asObject,
  asString,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

const SEARCH_RECENTS_STORAGE_KEY = "easyquran.search.recent";
const SEARCH_RECENTS_SCHEMA_VERSION = 1;
/** Kept queries per device, and the longest persisted query text. */
export const SEARCH_RECENTS_MAX = 10;
const SEARCH_RECENTS_QUERY_MAX = 128;

export interface SearchRecentEntry {
  readonly q: string;
  readonly at: number;
}

interface PersistedSearchRecents {
  v: number;
  entries: SearchRecentEntry[];
  paused: boolean;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asString/asNumber validate
function decodeEntry(raw: unknown): SearchRecentEntry | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  const q = asString(stored.q)?.trim() ?? "";
  const at = asNumber(stored.at, 0, Number.MAX_SAFE_INTEGER);
  if (q.length === 0 || q.length > SEARCH_RECENTS_QUERY_MAX || at === undefined) {
    return undefined;
  }
  return { q, at };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asArray validate
function decodeSearchRecents(raw: unknown): PersistedSearchRecents {
  const fallback: PersistedSearchRecents = {
    v: SEARCH_RECENTS_SCHEMA_VERSION,
    entries: [],
    paused: false,
  };
  if (isFutureSchema(raw, SEARCH_RECENTS_SCHEMA_VERSION)) return fallback;
  const stored = asObject(raw);
  if (!stored) return fallback;
  // Stored order IS recency order; decode drops duplicates and malformed entries.
  const seen = new Set<string>();
  const entries: SearchRecentEntry[] = [];
  for (const entry of asArray(stored.entries, decodeEntry)) {
    if (seen.has(entry.q)) continue;
    seen.add(entry.q);
    entries.push(entry);
    if (entries.length >= SEARCH_RECENTS_MAX) break;
  }
  return { v: SEARCH_RECENTS_SCHEMA_VERSION, entries, paused: stored.paused === true };
}

/**
 * Queries this device ran on the search page, most recent first. Local-only —
 * never synced, never sent. "Pause search history" stops recording but keeps
 * stored queries visible until cleared; blocked/full localStorage degrades to
 * memory-only (failed writes leave the in-memory list intact). Cross-tab via
 * the storage event.
 */
export class SearchRecentsStore {
  #entries = $state<SearchRecentEntry[]>([]);
  #paused = $state(false);
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    this.#load();
    this.#teardown = onStorageKey(SEARCH_RECENTS_STORAGE_KEY, () => this.#load());
  }

  #load(): void {
    const stored = decodeSearchRecents(readJSON(SEARCH_RECENTS_STORAGE_KEY));
    this.#entries = stored.entries;
    this.#paused = stored.paused;
  }

  #commit(entries: SearchRecentEntry[], paused: boolean): void {
    this.#entries = entries;
    this.#paused = paused;
    if (browser)
      writeJSON(SEARCH_RECENTS_STORAGE_KEY, {
        v: SEARCH_RECENTS_SCHEMA_VERSION,
        entries,
        paused,
      });
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  /** Queries, most recent first. */
  get queries(): readonly string[] {
    return this.#entries.map((entry) => entry.q);
  }

  get entries(): readonly SearchRecentEntry[] {
    return this.#entries;
  }

  get paused(): boolean {
    return this.#paused;
  }

  /**
   * Records one executed search: moves the query to the front, refreshes its
   * timestamp (consecutive repeats never duplicate), caps the list at
   * SEARCH_RECENTS_MAX. No-op while paused.
   */
  record(q: string, at: number = Date.now()): void {
    const query = q.trim();
    if (this.#paused || query.length === 0 || query.length > SEARCH_RECENTS_QUERY_MAX) return;
    const rest = this.#entries.filter((entry) => entry.q !== query);
    this.#commit([{ q: query, at }, ...rest].slice(0, SEARCH_RECENTS_MAX), this.#paused);
  }

  clear(): void {
    if (this.#entries.length === 0) return;
    this.#commit([], this.#paused);
  }

  setPaused(next: boolean): void {
    if (next === this.#paused) return;
    this.#commit(this.#entries, next);
  }
}

export const searchRecents = new SearchRecentsStore();
