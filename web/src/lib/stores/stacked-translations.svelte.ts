import { browser } from "$app/env";
import { uniq } from "es-toolkit";

import { STACKED_MAX_EXTRAS } from "#lib/data/quran-types.js";
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

export { STACKED_MAX_EXTRAS };

const STACKED_STORAGE_KEY = "easyquran.reader.stacked";
const STACKED_SCHEMA_VERSION = 1;

/** How the extra translations sit under a verse: one after another, or side by side. */
export type LaneLayout = "stacked" | "parallel";

/** Per-lane text size steps, from smallest to largest. Step 0 is the reader's size. */
export const LANE_STEP_MIN = -2;
export const LANE_STEP_MAX = 2;
/** Font scale per step, indexed by `step - LANE_STEP_MIN`. */
const LANE_SCALES: readonly number[] = [0.75, 0.875, 1, 1.125, 1.25];

interface PersistedStacked {
  v: number;
  ids: string[];
  /** Additive field under the same schema version: false pauses the flow while
   * keeping every selection (the modal's "Show translations" switch). */
  enabled?: boolean;
  /** Additive: absent means stacked. Written only when parallel. */
  layout?: LaneLayout;
  /** Additive: per-lane text size steps. Written only when some lane is stepped. */
  steps?: Record<string, number>;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); asObject/asNumber validate
function decodeSteps(raw: unknown, ids: readonly string[]) {
  const stored = asObject(raw);
  if (!stored) return {};
  const steps: Record<string, number> = {};
  for (const id of ids) {
    const step = asNumber(stored[id], LANE_STEP_MIN, LANE_STEP_MAX);
    if (step !== undefined && step !== 0) steps[id] = step;
  }
  return steps;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage JSON boundary (readJSON -> JSON.parse); isFutureSchema/asObject/asArray validate
function decodeStacked(raw: unknown): PersistedStacked {
  const fallback: PersistedStacked = { v: STACKED_SCHEMA_VERSION, ids: [] };
  if (isFutureSchema(raw, STACKED_SCHEMA_VERSION)) return fallback;
  const stored = asObject(raw);
  if (!stored) return fallback;
  const ids = asArray(stored.ids, asString);
  return {
    v: STACKED_SCHEMA_VERSION,
    ids,
    enabled: stored.enabled !== false,
    layout: stored.layout === "parallel" ? "parallel" : "stacked",
    steps: decodeSteps(stored.steps, ids),
  };
}

function sameIds(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((id, i) => id === b[i]);
}

function sameSteps(a: Record<string, number>, b: Record<string, number>): boolean {
  const keys = Object.keys(a);
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every((id) => a[id] === b[id]);
}

/** Steps only for lanes still selected, so a removed lane leaves no size behind. */
function stepsFor(ids: readonly string[], steps: Record<string, number>) {
  const kept: Record<string, number> = {};
  for (const id of ids) {
    const step = steps[id];
    if (step !== undefined && step !== 0) kept[id] = step;
  }
  return kept;
}

export class StackedTranslationsStore {
  #ids = $state<string[]>([]);
  #enabled = $state(true);
  #layout = $state<LaneLayout>("stacked");
  #steps = $state<Record<string, number>>({});
  #teardown: (() => void) | null = null;

  constructor() {
    if (!browser) return;
    const initial = decodeStacked(readJSON(STACKED_STORAGE_KEY));
    this.#ids = initial.ids;
    this.#enabled = initial.enabled ?? true;
    this.#layout = initial.layout ?? "stacked";
    this.#steps = initial.steps ?? {};
    this.#teardown = onStorageKey(STACKED_STORAGE_KEY, () => {
      const next = decodeStacked(readJSON(STACKED_STORAGE_KEY));
      if (!sameIds(next.ids, this.#ids)) this.#ids = next.ids;
      const nextEnabled = next.enabled ?? true;
      if (nextEnabled !== this.#enabled) this.#enabled = nextEnabled;
      const nextLayout = next.layout ?? "stacked";
      if (nextLayout !== this.#layout) this.#layout = nextLayout;
      const nextSteps = next.steps ?? {};
      if (!sameSteps(nextSteps, this.#steps)) this.#steps = nextSteps;
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

  /** How the extras sit under a verse. Client-only: no change to what is fetched. */
  get layout(): LaneLayout {
    return this.#layout;
  }

  /** One lane's size step (0 is the reader's size). */
  laneStep(id: string): number {
    return this.#steps[id] ?? 0;
  }

  /** The font scale for one lane (1 at step 0). */
  laneScale(id: string): number {
    return LANE_SCALES[this.laneStep(id) - LANE_STEP_MIN] ?? 1;
  }

  #commit(
    ids: string[],
    enabled: boolean = this.#enabled,
    layout: LaneLayout = this.#layout,
    steps: Record<string, number> = this.#steps,
  ): void {
    const kept = stepsFor(ids, steps);
    this.#ids = ids;
    this.#enabled = enabled;
    this.#layout = layout;
    this.#steps = kept;
    if (browser) {
      const record: PersistedStacked = { v: STACKED_SCHEMA_VERSION, ids, enabled };
      if (layout === "parallel") record.layout = layout;
      if (Object.keys(kept).length > 0) record.steps = kept;
      writeJSON(STACKED_STORAGE_KEY, record);
    }
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

  setLayout(layout: LaneLayout): void {
    if (layout === this.#layout) return;
    this.#commit([...this.#ids], this.#enabled, layout);
  }

  /** Make one selected lane a step larger (+1) or smaller (-1), clamped to the step range. */
  stepLane(id: string, delta: -1 | 1): void {
    if (!this.#ids.includes(id)) return;
    const current = this.#steps[id] ?? 0;
    const next = Math.min(LANE_STEP_MAX, Math.max(LANE_STEP_MIN, current + delta));
    if (next === current) return;
    this.#commit([...this.#ids], this.#enabled, this.#layout, { ...this.#steps, [id]: next });
  }

  resetLane(id: string): void {
    if (this.#steps[id] === undefined) return;
    const rest = Object.fromEntries(Object.entries(this.#steps).filter(([key]) => key !== id));
    this.#commit([...this.#ids], this.#enabled, this.#layout, rest);
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
