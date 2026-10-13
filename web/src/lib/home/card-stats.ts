import type { CardBase } from "./cards.svelte.js";

/**
 * Pure per-card statistics derived from the recite event ring (≤500 epoch-ms
 * stamps): calendar-window counts, streaks over device-LOCAL days, last recite,
 * and a 30-day activity strip. Local-day math uses the user's timezone via the
 * same `YYYY-MM-DD` day-key shape as `todayLocal()` in
 * `#lib/quran/engagement-state.js`; day-string comparison (never 24h gaps)
 * keeps DST-short/long days and timezone travel correct. `total` stays
 * `card.count` — the uncapped fast tally — which may legitimately exceed the
 * ring-derived windows for very heavy use or pre-ring (legacy) cards.
 */

const DAY_MS = 86_400_000;

export interface ActivityDay {
  readonly day: string;
  readonly count: number;
}

export interface CardStats {
  /** The card's uncapped fast tally (`card.count`), not ring-derived. */
  readonly total: number;
  readonly today: number;
  readonly last7: number;
  readonly last30: number;
  readonly currentStreak: number;
  readonly bestStreak: number;
  readonly lastRecitedAt: number | null;
  /** 30 local days ending today, oldest first; zero-count days included. */
  readonly activity30: readonly ActivityDay[];
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** Device-local `YYYY-MM-DD` for an epoch-ms instant (todayLocal() shape). */
export function dayKeyOf(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** UTC `YYYY-MM-DD` — only for day-key arithmetic (Date.parse round-trip). */
function utcDayKeyOf(ms: number): string {
  const d = new Date(ms);
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
}

/** Whole calendar days between two `YYYY-MM-DD` keys (UTC parse is DST-safe). */
function diffDays(laterKey: string, earlierKey: string): number {
  return (Date.parse(laterKey) - Date.parse(earlierKey)) / DAY_MS;
}

/** The n local calendar days ending today (or at `nowMs`), oldest first. */
function dayKeysEndingAt(nowMs: number, n: number): string[] {
  const keys: string[] = [];
  const cursor = new Date(nowMs);
  for (let i = 0; i < n; i += 1) {
    keys.unshift(dayKeyOf(cursor.getTime()));
    cursor.setDate(cursor.getDate() - 1);
  }
  return keys;
}

function windowCount(counts: ReadonlyMap<string, number>, days: readonly string[]): number {
  let sum = 0;
  for (const day of days) sum += counts.get(day) ?? 0;
  return sum;
}

/**
 * Streak still alive as of `nowMs`: consecutive local recite days counted back
 * from today, falling back to yesterday when today has no recite yet (the day
 * is not over — the grace rule).
 */
function currentStreak(days: readonly string[], todayKey: string): number {
  const last = days[days.length - 1];
  if (last === undefined) return 0;
  const yesterdayKey = utcDayKeyOf(Date.parse(todayKey) - DAY_MS);
  if (last !== todayKey && last !== yesterdayKey) return 0;
  let streak = 1;
  for (let i = days.length - 1; i > 0; i -= 1) {
    const current = days[i];
    const previous = days[i - 1];
    if (current === undefined || previous === undefined) break;
    if (diffDays(current, previous) !== 1) break;
    streak += 1;
  }
  return streak;
}

/** Longest consecutive-day run anywhere in the retained log. */
function bestStreak(days: readonly string[]): number {
  let best = 0;
  let run = 0;
  let previous: string | undefined;
  for (const day of days) {
    run = previous !== undefined && diffDays(day, previous) === 1 ? run + 1 : 1;
    if (run > best) best = run;
    previous = day;
  }
  return best;
}

/** Derives every statistic from the card's tally + event ring at `nowMs`. */
export function cardStats(card: Pick<CardBase, "count" | "events">, nowMs: number): CardStats {
  const events = card.events ?? [];
  const todayKey = dayKeyOf(nowMs);
  const counts = new Map<string, number>();
  let lastRecitedAt: number | null = null;
  for (const ms of events) {
    const key = dayKeyOf(ms);
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (lastRecitedAt === null || ms > lastRecitedAt) lastRecitedAt = ms;
  }
  const days = [...counts.keys()].sort();
  // Windows are local CALENDAR days ending today — never 24h subtractions, so
  // a DST shift inside the window cannot move the boundary a whole day.
  return {
    total: card.count,
    today: counts.get(todayKey) ?? 0,
    last7: windowCount(counts, dayKeysEndingAt(nowMs, 7)),
    last30: windowCount(counts, dayKeysEndingAt(nowMs, 30)),
    currentStreak: currentStreak(days, todayKey),
    bestStreak: bestStreak(days),
    lastRecitedAt,
    activity30: dayKeysEndingAt(nowMs, 30).map((day) => ({ day, count: counts.get(day) ?? 0 })),
  };
}
