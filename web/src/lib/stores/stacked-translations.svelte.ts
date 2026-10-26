import { browser } from "$app/env";
import { uniq } from "es-toolkit";

import { STACKED_MAX_EXTRAS } from "#lib/data/quran-types.js";
import {
  asArray,
  asObject,
  asString,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

export { STACKED_MAX_EXTRAS };

const STACKED_STORAGE_KEY = "easyquran.reader.stacked";
const STACKED_SCHEMA_VERSION = 1;

interface PersistedStacked {
  v: number;
  ids: string[];
  /** Additive field under the same schema version: false pauses the flow while
   * keeping every selection (the modal's "Show translations" switch). */
  enabled?: boolean;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); isFutureSchema/asObject/asArray validate
function decodeStacked(raw: unknown): PersistedStacked {
  const fallback: PersistedStacked = { v: STACKED_SCHEMA_VERSION, ids: [] };
  if (isFutureSchema(raw, STACKED_SCHEMA_VERSION)) return fallback;
  const stored = asObject(raw);
  if (!stored) return fallback;
  return {
    v: STACKED_SCHEMA_VERSION,
    ids: asArray(stored.ids, asString),
    enabled: stored.enabled !== false,
  };
}

function sameIds(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((id, i) => id === b[i]);
}

export class StackedTranslationsStore {
  #ids = $state<string[]>([]);
  #enabled = $state(true);
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    const initial = decodeStacked(readJSON(STACKED_STORAGE_KEY));
    this.#ids = initial.ids;
    this.#enabled = initial.enabled ?? true;
    this.#teardown = onStorageKey(STACKED_STORAGE_KEY, () => {
      const next = decodeStacked(readJSON(STACKED_STORAGE_KEY));
      if (!sameIds(next.ids, this.#ids)) this.#ids = next.ids;
      const nextEnabled = next.enabled ?? true;
      if (nextEnabled !== this.#enabled) this.#enabled = nextEnabled;
    });
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  get ids(): readonly string[] {
    return this.#ids;
  }

  /** Whether the reader flows the selection at all. Selections survive a pause. */
  get enabled(): boolean {
    return this.#enabled;
  }

  /** What the reader renders right now: the selection when on, nothing when paused. */
  get activeIds(): readonly string[] {
    return this.#enabled ? this.#ids : [];
  }

  #commit(ids: string[], enabled: boolean = this.#enabled): void {
    this.#ids = ids;
    this.#enabled = enabled;
    if (browser) writeJSON(STACKED_STORAGE_KEY, { v: STACKED_SCHEMA_VERSION, ids, enabled });
  }

  setIds(ids: readonly string[]): void {
    const next = uniq(ids).slice(0, STACKED_MAX_EXTRAS);
    if (sameIds(next, this.#ids)) return;
    this.#commit(next);
  }

  setEnabled(enabled: boolean): void {
    if (enabled === this.#enabled) return;
    this.#commit([...this.#ids], enabled);
  }

  toggleEnabled(): void {
    this.setEnabled(!this.#enabled);
  }

  toggle(id: string): void {
    const next = this.#ids.includes(id) ? this.#ids.filter((x) => x !== id) : [...this.#ids, id];
    this.setIds(next);
  }

  reorder(id: string, delta: -1 | 1): void {
    const i = this.#ids.indexOf(id);
    if (i === -1) return;
    const j = i + delta;
    if (j < 0 || j >= this.#ids.length) return;
    const fromI = this.#ids[i];
    const fromJ = this.#ids[j];
    if (fromI === undefined || fromJ === undefined) return;
    const next = this.#ids.slice();
    next[i] = fromJ;
    next[j] = fromI;
    if (sameIds(next, this.#ids)) return;
    this.#commit(next);
  }

  remove(id: string): void {
    if (!this.#ids.includes(id)) return;
    this.#commit(this.#ids.filter((x) => x !== id));
  }

  clear(): void {
    if (this.#ids.length === 0) return;
    this.#commit([]);
  }
}

export const stackedTranslations = new StackedTranslationsStore();
