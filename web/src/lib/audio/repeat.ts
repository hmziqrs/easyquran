/**
 * F20 memorization repeat engine. Source-agnostic: it schedules ayah plays and
 * knows nothing about audio. The audio engine supplies `play` and `wait`.
 */
import type { JsonRecord, JsonValue } from "#lib/storage/decoders.js";
import {
  asArray,
  asNumber,
  asObject,
  asString,
  readJSON,
  tryWriteJSON,
} from "#lib/storage/index.js";

import { MAX_REPEAT_STEPS, REPEAT_LIMITS, clampInt } from "./policy";
import { isSurahNumber } from "./url";

export interface RepeatPlan {
  readonly surah: number;
  /** First ayah of the range, inclusive. */
  readonly from: number;
  /** Last ayah of the range, inclusive. Never beyond the surah. */
  readonly to: number;
  /** Plays of each ayah before moving on. */
  readonly ayahRepeat: number;
  /** Passes over the whole range. */
  readonly setRepeat: number;
  /** Silence between consecutive plays, in milliseconds. */
  readonly delayMs: number;
}

export interface RepeatPlanInput {
  readonly surah: number;
  readonly from: number;
  readonly to: number;
  readonly ayahRepeat: number;
  readonly setRepeat: number;
  readonly delayMs: number;
}

export interface RepeatStep {
  readonly surah: number;
  readonly ayah: number;
  /** Set pass, 1-based. */
  readonly pass: number;
  readonly passes: number;
  /** Repeat of this ayah within the pass, 1-based. */
  readonly ayahPass: number;
  readonly ayahPasses: number;
  /** Zero-based position in the whole run. */
  readonly ordinal: number;
  readonly total: number;
}

/**
 * Validates and clamps a plan against the surah's real ayah count. Null when the
 * surah is invalid, the range is empty or inverted, or the run exceeds MAX_REPEAT_STEPS.
 * `to` is clamped down to `ayahCount` (boundary), `from` must already fit inside it.
 */
export function normalizePlan(input: RepeatPlanInput, ayahCount: number): RepeatPlan | null {
  if (!isSurahNumber(input.surah) || !Number.isInteger(ayahCount) || ayahCount < 1) return null;
  if (!Number.isFinite(input.from) || !Number.isFinite(input.to)) return null;
  const from = Math.round(input.from);
  const to = Math.min(Math.round(input.to), ayahCount);
  if (from < 1 || from > to) return null;
  const plan: RepeatPlan = {
    surah: input.surah,
    from,
    to,
    ayahRepeat: clampInt(input.ayahRepeat, REPEAT_LIMITS.ayahRepeat),
    setRepeat: clampInt(input.setRepeat, REPEAT_LIMITS.setRepeat),
    delayMs: clampInt(input.delayMs, REPEAT_LIMITS.delayMs),
  };
  if (planLength(plan) > MAX_REPEAT_STEPS) return null;
  return plan;
}

/** Number of plays the plan schedules. */
export function planLength(plan: RepeatPlan): number {
  return plan.setRepeat * (plan.to - plan.from + 1) * plan.ayahRepeat;
}

/** The full play sequence: set passes outer, ayahs in order, per-ayah repeats inner. */
export function planSteps(plan: RepeatPlan): RepeatStep[] {
  const total = planLength(plan);
  const steps: RepeatStep[] = [];
  for (let pass = 1; pass <= plan.setRepeat; pass++) {
    for (let ayah = plan.from; ayah <= plan.to; ayah++) {
      for (let ayahPass = 1; ayahPass <= plan.ayahRepeat; ayahPass++) {
        steps.push({
          surah: plan.surah,
          ayah,
          pass,
          passes: plan.setRepeat,
          ayahPass,
          ayahPasses: plan.ayahRepeat,
          ordinal: steps.length,
          total,
        });
      }
    }
  }
  return steps;
}

export interface RepeatIo {
  /** Plays one step. Resolves when it finishes. Throws when it cannot play. */
  play(step: RepeatStep, signal: AbortSignal): Promise<void>;
  /** Resolves after `ms`, or early when the signal aborts. */
  wait(ms: number, signal: AbortSignal): Promise<void>;
  onStep?(step: RepeatStep): void;
}

/**
 * Runs the steps in order with `delayMs` between consecutive plays. Never waits
 * after the last step. Returns "cancelled" as soon as the signal aborts, with no
 * further play or wait started.
 */
export async function runRepeat(
  steps: readonly RepeatStep[],
  delayMs: number,
  io: RepeatIo,
  signal: AbortSignal,
): Promise<"done" | "cancelled"> {
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    if (!step || signal.aborted) return "cancelled";
    io.onStep?.(step);
    await io.play(step, signal);
    if (signal.aborted) return "cancelled";
    if (i < steps.length - 1 && delayMs > 0) {
      await io.wait(delayMs, signal);
      if (signal.aborted) return "cancelled";
    }
  }
  return "done";
}

/** Abortable sleep. Resolves early on abort, never rejects. */
export function waitMs(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve();
      return;
    }
    const finish = (): void => {
      clearTimeout(timer);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    const timer = setTimeout(finish, Math.max(0, ms));
    signal.addEventListener("abort", finish, { once: true });
  });
}

/**
 * Keeps the screen awake while a repeat runs, where the browser offers the Wake
 * Lock API. Re-requests when the tab becomes visible again (the browser drops the
 * lock on hide). Returns the release function. A no-op where unsupported.
 */
export function holdScreenAwake(): () => void {
  if (!("wakeLock" in navigator)) return () => {};
  let sentinel: WakeLockSentinel | null = null;
  let held = true;
  const acquire = (): void => {
    if (!held || document.visibilityState !== "visible") return;
    navigator.wakeLock
      .request("screen")
      .then((lock) => {
        if (held) sentinel = lock;
        else void lock.release().catch(() => {});
      })
      .catch(() => {});
  };
  document.addEventListener("visibilitychange", acquire);
  acquire();
  return () => {
    held = false;
    document.removeEventListener("visibilitychange", acquire);
    const lock = sentinel;
    sentinel = null;
    if (lock) void lock.release().catch(() => {});
  };
}

// ---------------------------------------------------------------------------
// Saved presets (device-local, per-viewer convenience).
// ---------------------------------------------------------------------------

export interface RepeatPreset extends RepeatPlanInput {
  readonly id: string;
  readonly name: string;
}

export const REPEAT_PRESETS_KEY = "easyquran.audio.repeatPresets.v1";
export const MAX_REPEAT_PRESETS = 20;
const MAX_PRESET_NAME = 60;

/** Decodes one stored preset. Undefined when any field is missing or out of range. */
function decodePreset(obj: JsonRecord | undefined): RepeatPreset | undefined {
  if (!obj) return undefined;
  const id = asString(obj.id);
  const name = asString(obj.name);
  const surah = asNumber(obj.surah, 1, 114);
  const from = asNumber(obj.from, 1, 286);
  const to = asNumber(obj.to, 1, 286);
  const ayahRepeat = asNumber(
    obj.ayahRepeat,
    REPEAT_LIMITS.ayahRepeat.min,
    REPEAT_LIMITS.ayahRepeat.max,
  );
  const setRepeat = asNumber(
    obj.setRepeat,
    REPEAT_LIMITS.setRepeat.min,
    REPEAT_LIMITS.setRepeat.max,
  );
  const delayMs = asNumber(obj.delayMs, REPEAT_LIMITS.delayMs.min, REPEAT_LIMITS.delayMs.max);
  if (
    id === undefined ||
    name === undefined ||
    name.length === 0 ||
    name.length > MAX_PRESET_NAME ||
    surah === undefined ||
    from === undefined ||
    to === undefined ||
    ayahRepeat === undefined ||
    setRepeat === undefined ||
    delayMs === undefined
  ) {
    return undefined;
  }
  return { id, name, surah, from, to, ayahRepeat, setRepeat, delayMs };
}

/** Keeps only well-formed presets, capped at MAX_REPEAT_PRESETS. Unknown input yields []. */
export function parsePresets(raw: JsonValue | undefined): RepeatPreset[] {
  return asArray(raw, (item) => decodePreset(asObject(item))).slice(0, MAX_REPEAT_PRESETS);
}

export function loadRepeatPresets(): RepeatPreset[] {
  return parsePresets(readJSON(REPEAT_PRESETS_KEY));
}

/** False when storage refuses the write. The caller then shows that the preset did not save. */
export function saveRepeatPresets(presets: readonly RepeatPreset[]): boolean {
  return tryWriteJSON(REPEAT_PRESETS_KEY, presets);
}

/** Adds a preset at the front. Past the cap, the oldest one drops off. */
export function addPreset(presets: readonly RepeatPreset[], preset: RepeatPreset): RepeatPreset[] {
  return [preset, ...presets.filter((p) => p.id !== preset.id)].slice(0, MAX_REPEAT_PRESETS);
}

export function removePreset(presets: readonly RepeatPreset[], id: string): RepeatPreset[] {
  return presets.filter((p) => p.id !== id);
}
