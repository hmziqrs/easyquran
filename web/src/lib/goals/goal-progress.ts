import { asLiteral, asNumber, asObject, asString } from "#lib/storage/decoders.js";

/**
 * Reading goals and streaks (F8). Pure: days in, numbers out. The rules are in
 * contracts/sync.md ("Reading goals"). Days are device-local `YYYY-MM-DD` strings
 * taken from the reads themselves, so no clock or zone is applied here. Date
 * arithmetic runs on UTC midnights, which keeps DST out of day counts.
 */

/** Verses in the Quran: the target of a finish goal, counted in verse reads. */
export const QURAN_AYAH_TOTAL = 6236;
/** Goals one account may hold. Enforced by the sync validator too (`goals.reading`). */
export const GOAL_LIMIT = 10;
export const DAILY_MINUTES_MIN = 1;
export const DAILY_MINUTES_MAX = 600;
/** Widest goal window, a year plus slack. Matches the server's days query span. */
export const GOAL_SPAN_MAX_DAYS = 400;
const MS_PER_DAY = 86_400_000;
const ID_LENGTH = 36;
const DAY_RE = /^\d{4}-\d{2}-\d{2}$/u;

export const GOAL_KINDS = ["daily_minutes", "finish_quran"] as const;
export type GoalKind = (typeof GOAL_KINDS)[number];

export interface ReadingGoal {
  readonly id: string;
  readonly kind: GoalKind;
  /** Minutes per day for `daily_minutes`; null for `finish_quran`. */
  readonly dailyMinutes: number | null;
  readonly startDay: string;
  /** Last day of the window before pauses, inclusive. */
  readonly endDay: string;
  /** Day the goal was paused, or null while it runs. */
  readonly pausedFrom: string | null;
  /** Paused days already added to the window. */
  readonly pausedDays: number;
}

/** One day from the server rollup (or this device's rollup when signed out). */
export interface ReadingDay {
  readonly localDay: string;
  readonly ayahsRead: number;
  readonly ms: number;
}

export type GoalState = "active" | "paused" | "completed" | "ended";

export interface GoalProgress {
  readonly done: number;
  readonly target: number;
  readonly state: GoalState;
}

/** `YYYY-MM-DD` → UTC midnight, or NaN when the day is not a real date. */
function utcMidnight(day: string): number {
  if (!DAY_RE.test(day)) return Number.NaN;
  const [y, m, d] = day.split("-").map(Number);
  const time = Date.UTC(y ?? 0, (m ?? 1) - 1, d ?? 1);
  return new Date(time).getUTCDate() === d ? time : Number.NaN;
}

export function isDay(day: string): boolean {
  return !Number.isNaN(utcMidnight(day));
}

/** Whole days from `from` to `to` (negative when `to` is earlier). */
export function daysBetween(from: string, to: string): number {
  return Math.round((utcMidnight(to) - utcMidnight(from)) / MS_PER_DAY);
}

export function addDays(day: string, count: number): string {
  return new Date(utcMidnight(day) + count * MS_PER_DAY).toISOString().slice(0, 10);
}

/**
 * Minutes a day counts toward a time goal. A verse read always counts at least
 * one minute, so a fast reader who covers many verses is never shortchanged.
 */
export function creditedMinutes(day: ReadingDay): number {
  return Math.max(day.ms / 60_000, day.ayahsRead);
}

/**
 * Last day of the window as of `today`. Paused days never use up the window: the
 * end moves on by each day a pause has run, the same as a resume would add them.
 */
export function windowEnd(goal: ReadingGoal, today: string): string {
  const paused = goal.pausedFrom === null ? 0 : Math.max(0, daysBetween(goal.pausedFrom, today));
  return addDays(goal.endDay, goal.pausedDays + paused);
}

/** Total the goal asks for. A time goal is minutes per day over its base span. */
export function goalTarget(goal: ReadingGoal): number {
  if (goal.kind === "finish_quran") return QURAN_AYAH_TOTAL;
  const span = daysBetween(goal.startDay, goal.endDay) + 1;
  return (goal.dailyMinutes ?? 0) * span;
}

export function goalProgress(
  goal: ReadingGoal,
  days: readonly ReadingDay[],
  today: string,
): GoalProgress {
  const end = windowEnd(goal, today);
  let done = 0;
  for (const day of days) {
    if (day.localDay < goal.startDay || day.localDay > end) continue;
    done += goal.kind === "finish_quran" ? day.ayahsRead : creditedMinutes(day);
  }
  const target = goalTarget(goal);
  let state: GoalState = "active";
  if (done >= target) state = "completed";
  else if (goal.pausedFrom !== null) state = "paused";
  else if (today > end) state = "ended";
  return { done, target, state };
}

/**
 * Consecutive days with at least one verse read, counting back from today. If
 * today has no reads yet, counting starts at yesterday, so a streak is not broken
 * before the day is over.
 */
export function streakOf(days: readonly ReadingDay[], today: string): number {
  const readDays = new Set(days.filter((d) => d.ayahsRead > 0).map((d) => d.localDay));
  let cursor = readDays.has(today) ? today : addDays(today, -1);
  let count = 0;
  while (readDays.has(cursor)) {
    count += 1;
    cursor = addDays(cursor, -1);
  }
  return count;
}

/** The `weeks * 7` days ending today, oldest first, for a calendar grid. */
export function calendarDays(today: string, weeks: number): string[] {
  const out: string[] = [];
  for (let back = weeks * 7 - 1; back >= 0; back -= 1) out.push(addDays(today, -back));
  return out;
}

export interface GoalDraft {
  readonly kind: GoalKind;
  readonly dailyMinutes: number | null;
  readonly startDay: string;
  readonly endDay: string;
}

export type GoalDraftError = "dates" | "span" | "minutes" | "limit";

/** Checks a goal before it is stored. Returns the first problem, or null. */
export function draftError(draft: GoalDraft, existing: number): GoalDraftError | null {
  if (existing >= GOAL_LIMIT) return "limit";
  if (!isDay(draft.startDay) || !isDay(draft.endDay)) return "dates";
  const span = daysBetween(draft.startDay, draft.endDay);
  if (span < 0) return "dates";
  if (span + 1 > GOAL_SPAN_MAX_DAYS) return "span";
  if (draft.kind === "daily_minutes") {
    const minutes = draft.dailyMinutes;
    if (
      minutes === null ||
      !Number.isInteger(minutes) ||
      minutes < DAILY_MINUTES_MIN ||
      minutes > DAILY_MINUTES_MAX
    ) {
      return "minutes";
    }
  }
  return null;
}

/** A goal from a checked draft. `id` is the caller's UUID v7. */
export function goalFromDraft(draft: GoalDraft, id: string): ReadingGoal {
  return {
    id,
    kind: draft.kind,
    dailyMinutes: draft.kind === "daily_minutes" ? draft.dailyMinutes : null,
    startDay: draft.startDay,
    endDay: draft.endDay,
    pausedFrom: null,
    pausedDays: 0,
  };
}

export const GOAL_PRESETS = ["minutes_30", "finish_30", "finish_365"] as const;
export type GoalPreset = (typeof GOAL_PRESETS)[number];

/** The three presets: 15 minutes a day for 30 days, finish in 30 days, finish in a year. */
export function presetDraft(preset: GoalPreset, today: string): GoalDraft {
  if (preset === "minutes_30") {
    return { kind: "daily_minutes", dailyMinutes: 15, startDay: today, endDay: addDays(today, 29) };
  }
  const span = preset === "finish_30" ? 30 : 365;
  return {
    kind: "finish_quran",
    dailyMinutes: null,
    startDay: today,
    endDay: addDays(today, span - 1),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed goal row from storage or a synced document; this decoder is its boundary parser.
function decodeGoal(raw: unknown): ReadingGoal | undefined {
  const o = asObject(raw);
  if (o === undefined) return undefined;
  const id = asString(o.id);
  const kind = asLiteral<GoalKind>(o.kind, GOAL_KINDS);
  const startDay = asString(o.startDay) ?? "";
  const endDay = asString(o.endDay) ?? "";
  const pausedFrom = o.pausedFrom === null ? null : asString(o.pausedFrom);
  const pausedDays = asNumber(o.pausedDays, 0, GOAL_SPAN_MAX_DAYS);
  const minutes =
    kind === "daily_minutes"
      ? (asNumber(o.dailyMinutes, DAILY_MINUTES_MIN, DAILY_MINUTES_MAX) ?? null)
      : null;
  if (id === undefined || id.length !== ID_LENGTH || kind === undefined) return undefined;
  if (pausedFrom === undefined || pausedDays === undefined) return undefined;
  if (pausedFrom !== null && !isDay(pausedFrom)) return undefined;
  const draft: GoalDraft = { kind, dailyMinutes: minutes, startDay, endDay };
  if (draftError(draft, 0) !== null) return undefined;
  return { ...goalFromDraft(draft, id), pausedFrom, pausedDays };
}

/**
 * Forward-compatible decode of `{goals: [...]}`: bad rows drop, the limit holds,
 * nothing throws. Used for both the local cache and a synced document.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the untyped localStorage or synced JSON; this function is the boundary parser.
export function decodeGoals(raw: unknown): readonly ReadingGoal[] {
  const list = asObject(raw)?.goals;
  if (!Array.isArray(list)) return [];
  const out: ReadingGoal[] = [];
  const seen = new Set<string>();
  for (const row of list) {
    const goal = decodeGoal(row);
    if (goal === undefined || seen.has(goal.id)) continue;
    seen.add(goal.id);
    out.push(goal);
    if (out.length >= GOAL_LIMIT) break;
  }
  return out;
}
