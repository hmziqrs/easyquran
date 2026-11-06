import { browser } from "$app/env";

import { parseVerseKey } from "#lib/bookmarks/schema.js";
import {
  asArray,
  asObject,
  asString,
  isFutureSchema,
  onStorageKey,
  readJSON,
  writeJSON,
} from "#lib/storage/index.js";

const PINS_STORAGE_KEY = "easyquran.reader.pins";
const PINS_SCHEMA_VERSION = 1;
/** Most verses one device keeps pinned at once. */
export const PINS_MAX = 20;
/** Ayah text is short; a longer string is malformed storage, not a verse. */
const PIN_TEXT_MAX = 8000;
const PIN_LANES_MAX = 5;

/** Compare-view selection id for the Arabic text. Translation ids never collide with it. */
export const ARABIC_SELECTION = "arabic";

export interface PinnedLane {
  readonly sourceId: string;
  readonly name: string;
  readonly text: string;
}

/**
 * One pinned verse, device-local (never synced). `text` is the main line the row showed
 * when pinned: the Arabic on the Arabic reader (`sourceId` null), or the route translation's
 * text on a translation route. `lanes` snapshots the stacked translations shown then.
 */
export interface PinnedVerse {
  readonly key: string;
  readonly sourceId: string | null;
  readonly text: string;
  readonly lanes: readonly PinnedLane[];
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asString validate
function decodeLane(raw: unknown): PinnedLane | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  const sourceId = asString(stored.sourceId)?.trim() ?? "";
  const name = asString(stored.name) ?? "";
  const text = asString(stored.text);
  if (sourceId === "" || text === undefined || text.length > PIN_TEXT_MAX) return undefined;
  return { sourceId, name, text };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asString/asArray validate
function decodePin(raw: unknown): PinnedVerse | undefined {
  const stored = asObject(raw);
  if (!stored) return undefined;
  const key = asString(stored.key) ?? "";
  if (parseVerseKey(key) === null) return undefined;
  const sourceId = asString(stored.sourceId) ?? null;
  const text = asString(stored.text);
  if (text === undefined || text.length > PIN_TEXT_MAX) return undefined;
  const lanes = asArray(stored.lanes, decodeLane).slice(0, PIN_LANES_MAX);
  return { key, sourceId: sourceId === "" ? null : sourceId, text, lanes };
}

/** Stored order IS pin order. Decode drops malformed rows and duplicate keys. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); isFutureSchema/asObject/asArray validate
export function decodePins(raw: unknown): PinnedVerse[] {
  if (isFutureSchema(raw, PINS_SCHEMA_VERSION)) return [];
  const stored = asObject(raw);
  if (!stored) return [];
  const seen = new Set<string>();
  const pins: PinnedVerse[] = [];
  for (const pin of asArray(stored.pins, decodePin)) {
    if (seen.has(pin.key)) continue;
    seen.add(pin.key);
    pins.push(pin);
    if (pins.length >= PINS_MAX) break;
  }
  return pins;
}

export type PinToggleResult = "added" | "removed" | "full";

/**
 * Pinned verses on this device, in pin order. Local-only: no sync, nothing sent. Cross-tab
 * through the storage event. Blocked or full storage keeps the in-memory list.
 */
export class PinsStore {
  #pins = $state<PinnedVerse[]>([]);
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    this.#pins = decodePins(readJSON(PINS_STORAGE_KEY));
    this.#teardown = onStorageKey(PINS_STORAGE_KEY, () => {
      this.#pins = decodePins(readJSON(PINS_STORAGE_KEY));
    });
  }

  dispose(): void {
    this.#teardown?.();
    this.#teardown = null;
  }

  get list(): readonly PinnedVerse[] {
    return this.#pins;
  }

  get count(): number {
    return this.#pins.length;
  }

  has(key: string): boolean {
    return this.#pins.some((pin) => pin.key === key);
  }

  /** Pin the verse, or unpin it when already pinned. A full set refuses new pins. */
  toggle(verse: PinnedVerse): PinToggleResult {
    if (this.has(verse.key)) {
      this.remove(verse.key);
      return "removed";
    }
    if (this.#pins.length >= PINS_MAX) return "full";
    this.#commit([...this.#pins, verse]);
    return "added";
  }

  remove(key: string): void {
    if (!this.has(key)) return;
    this.#commit(this.#pins.filter((pin) => pin.key !== key));
  }

  /** Move one pin up (-1) or down (1). Edges are a no-op. */
  move(key: string, delta: -1 | 1): void {
    const i = this.#pins.findIndex((pin) => pin.key === key);
    const j = i + delta;
    if (i === -1 || j < 0 || j >= this.#pins.length) return;
    const next = this.#pins.slice();
    const moved = next[i];
    const other = next[j];
    if (moved === undefined || other === undefined) return;
    next[i] = other;
    next[j] = moved;
    this.#commit(next);
  }

  clear(): void {
    if (this.#pins.length === 0) return;
    this.#commit([]);
  }

  #commit(pins: PinnedVerse[]): void {
    this.#pins = pins;
    if (browser) writeJSON(PINS_STORAGE_KEY, { v: PINS_SCHEMA_VERSION, pins });
  }
}

/** One compare cell: what a pin shows under the chosen selection. */
export interface CompareCell {
  readonly key: string;
  readonly arabic: string | null;
  readonly translation: string | null;
}

/**
 * Selection ids the compare view offers: Arabic first, then each translation the pins carry
 * (route translation or stacked lane), in first-seen order.
 */
export function compareSelections(pins: readonly PinnedVerse[]): string[] {
  const ids = new Set<string>();
  for (const pin of pins) {
    if (pin.sourceId !== null) ids.add(pin.sourceId);
    for (const lane of pin.lanes) ids.add(lane.sourceId);
  }
  return [ARABIC_SELECTION, ...ids];
}

/** Compose one pin's compare cell for the selection. Text is the pinned text, unchanged. */
export function compareCell(pin: PinnedVerse, selection: string): CompareCell {
  const arabic = pin.sourceId === null ? pin.text : null;
  if (selection === ARABIC_SELECTION) return { key: pin.key, arabic, translation: null };
  if (pin.sourceId === selection) return { key: pin.key, arabic, translation: pin.text };
  const lane = pin.lanes.find((l) => l.sourceId === selection);
  return { key: pin.key, arabic, translation: lane?.text ?? null };
}

export const pins = new PinsStore();
