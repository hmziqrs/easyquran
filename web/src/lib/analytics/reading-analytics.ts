import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { todayLocal } from "#lib/quran/engagement-state.js";
import {
  asArray,
  asNumber,
  asObject,
  asString,
  onPageHide,
  readJSON,
  removeJSON,
  trailingDebounce,
  writeJSON,
} from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";

/**
 * Reading insights (docs/reading-model.md §4) — always on, personal only. The
 * read tracker reports each verse actually read with the active time it got;
 * events queue durably per owner under `easyquran.analytics.queue.{owner}`
 * and flush in idempotent batches (client event id) to
 * `POST /user/analytics/v1/events`. Signed out, events also roll up locally so
 * personal stats work without an account; on sign-in the device queue joins
 * the account's and uploads — the server summary then answers.
 * Google Analytics never sees any of this.
 */

const ROLLUP_KEY = "easyquran.analytics.local";
const QUEUE_KEY_PREFIX = "easyquran.analytics.queue.";
const LOCAL_OWNER = "local";
/** Server caps one batch at 100 events (user_analytics_v1 validator). */
const BATCH_MAX = 100;
/** Durable queue bound — the oldest events fall off past it. */
const QUEUE_CAP = 2000;
const ROLLUP_DAY_CAP = 60;
const MS_MAX = 3_600_000;
const SOURCE_ID_RE = /^[a-z0-9._-]{1,64}$/u;

export interface ReadingEvent {
  readonly id: string;
  readonly sessionId: string | null;
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
  readonly ms: number;
  readonly localDay: string;
  readonly at: string;
}

/** `easyquran.analytics.local` v2: days[localDay]["S:A"] = [reads, ms]. */
export interface LocalRollup {
  readonly v: 2;
  readonly days: Record<string, Record<string, readonly [number, number]>>;
}

export interface ReadingSummary {
  readonly totals: { readonly ayahs: number; readonly ms: number; readonly days: number };
  readonly topSurahs: readonly {
    readonly surah: number;
    readonly reads: number;
    readonly ms: number;
  }[];
  readonly topAyahs: readonly {
    readonly surah: number;
    readonly ayah: number;
    readonly reads: number;
    readonly ms: number;
  }[];
  readonly days: readonly {
    readonly localDay: string;
    readonly ayahsRead: number;
    readonly ms: number;
  }[];
}

function queueKeyFor(owner: string): string {
  return `${QUEUE_KEY_PREFIX}${owner}`;
}

function currentOwner(): string {
  if (authState.status !== "authenticated" || authState.user === null) return LOCAL_OWNER;
  return String(authState.user.id);
}

function emptyRollup(): LocalRollup {
  return { v: 2, days: {} };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed localStorage rollup; this decoder is its boundary parser.
export function decodeLocalRollup(raw: unknown): LocalRollup {
  const stored = asObject(raw);
  if (!stored || stored.v !== 2) return emptyRollup();
  const days: Record<string, Record<string, readonly [number, number]>> = {};
  for (const [day, versesRaw] of Object.entries(asObject(stored.days) ?? {})) {
    if (!/^\d{4}-\d{2}-\d{2}$/u.test(day)) continue;
    const verses: Record<string, readonly [number, number]> = {};
    for (const [key, pair] of Object.entries(asObject(versesRaw) ?? {})) {
      if (!/^\d{1,3}:\d{1,3}$/u.test(key) || !Array.isArray(pair)) continue;
      const reads = asNumber(pair[0], 1, 1_000_000);
      const ms = asNumber(pair[1], 0, Number.MAX_SAFE_INTEGER);
      if (reads !== undefined && ms !== undefined) verses[key] = [reads, ms];
    }
    days[day] = verses;
  }
  return { v: 2, days };
}

/** Same shape as the server's `/summary`, computed from the local rollup. */
export function summarizeRollup(rollup: LocalRollup): ReadingSummary {
  const perSurah = new Map<number, { reads: number; ms: number }>();
  const perAyah = new Map<string, { reads: number; ms: number }>();
  const days: { localDay: string; ayahsRead: number; ms: number }[] = [];
  let ayahs = 0;
  let totalMs = 0;
  for (const [localDay, verses] of Object.entries(rollup.days)) {
    let dayReads = 0;
    let dayMs = 0;
    for (const [key, [reads, ms]] of Object.entries(verses)) {
      const surah = Number(key.split(":")[0]);
      dayReads += reads;
      dayMs += ms;
      const s = perSurah.get(surah) ?? { reads: 0, ms: 0 };
      perSurah.set(surah, { reads: s.reads + reads, ms: s.ms + ms });
      const a = perAyah.get(key) ?? { reads: 0, ms: 0 };
      perAyah.set(key, { reads: a.reads + reads, ms: a.ms + ms });
    }
    ayahs += dayReads;
    totalMs += dayMs;
    days.push({ localDay, ayahsRead: dayReads, ms: dayMs });
  }
  return {
    totals: { ayahs, ms: totalMs, days: days.length },
    topSurahs: [...perSurah.entries()]
      .sort((a, b) => b[1].reads - a[1].reads || a[0] - b[0])
      .slice(0, 5)
      .map(([surah, v]) => ({ surah, ...v })),
    topAyahs: [...perAyah.entries()]
      .sort((a, b) => b[1].reads - a[1].reads || a[0].localeCompare(b[0]))
      .slice(0, 5)
      .map(([key, v]) => {
        const [surah, ayah] = key.split(":");
        return { surah: Number(surah), ayah: Number(ayah), ...v };
      }),
    days: days.sort((a, b) => b.localDay.localeCompare(a.localDay)).slice(0, 30),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed summary row; this decoder is its boundary parser.
function decodeSurahRow(raw: unknown) {
  const o = asObject(raw);
  const surah = asNumber(o?.surah, 1, 114);
  const reads = asNumber(o?.reads, 0, Number.MAX_SAFE_INTEGER);
  if (surah === undefined || reads === undefined) return undefined;
  return { surah, reads, ms: asNumber(o?.ms, 0, Number.MAX_SAFE_INTEGER) ?? 0 };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed GET /user/analytics/v1/summary body; this decoder is its boundary parser.
export function decodeSummary(raw: unknown): ReadingSummary | null {
  const o = asObject(raw);
  if (!o) return null;
  const totals = asObject(o.totals);
  return {
    totals: {
      ayahs: asNumber(totals?.ayahs, 0, Number.MAX_SAFE_INTEGER) ?? 0,
      ms: asNumber(totals?.ms, 0, Number.MAX_SAFE_INTEGER) ?? 0,
      days: asNumber(totals?.days, 0, Number.MAX_SAFE_INTEGER) ?? 0,
    },
    topSurahs: asArray(o.topSurahs, (row) => decodeSurahRow(row)),
    topAyahs: asArray(o.topAyahs, (row) => {
      const base = decodeSurahRow(row);
      const ayah = asNumber(asObject(row)?.ayah, 1, 286);
      return base === undefined || ayah === undefined ? undefined : { ...base, ayah };
    }),
    days: asArray(o.days, (row) => {
      const d = asObject(row);
      const localDay = asString(d?.localDay);
      if (localDay === undefined) return undefined;
      return {
        localDay,
        ayahsRead: asNumber(d?.ayahsRead, 0, Number.MAX_SAFE_INTEGER) ?? 0,
        ms: asNumber(d?.ms, 0, Number.MAX_SAFE_INTEGER) ?? 0,
      };
    }),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed queued event; this decoder is its boundary parser.
function decodeEvent(raw: unknown): ReadingEvent | undefined {
  const o = asObject(raw);
  if (!o) return undefined;
  const id = asString(o.id);
  const surah = asNumber(o.surah, 1, 114);
  const ayah = asNumber(o.ayah, 1, 286);
  const ms = asNumber(o.ms, 0, MS_MAX);
  const localDay = asString(o.localDay);
  const at = asString(o.at);
  if (
    id === undefined ||
    surah === undefined ||
    ayah === undefined ||
    ms === undefined ||
    localDay === undefined ||
    at === undefined
  ) {
    return undefined;
  }
  const sessionId = asString(o.sessionId);
  const sourceId = asString(o.sourceId);
  return {
    id,
    sessionId: sessionId !== undefined && sessionId.length === 36 ? sessionId : null,
    surah,
    ayah,
    sourceId: sourceId !== undefined && SOURCE_ID_RE.test(sourceId) ? sourceId : null,
    ms,
    localDay,
    at,
  };
}

function readQueue(owner: string): ReadingEvent[] {
  return asArray(asObject(readJSON(queueKeyFor(owner)))?.events, (row) => decodeEvent(row));
}

function writeQueue(owner: string, events: readonly ReadingEvent[]): void {
  if (events.length === 0) {
    removeJSON(queueKeyFor(owner));
    return;
  }
  // FIFO trim: when the cap is hit the OLDEST events fall off, never the new one.
  writeJSON(queueKeyFor(owner), { v: 2, events: events.slice(-QUEUE_CAP) });
}

function recordLocal(event: ReadingEvent): void {
  const rollup = decodeLocalRollup(readJSON(ROLLUP_KEY));
  const days = { ...rollup.days };
  const day = { ...days[event.localDay] };
  const key = `${event.surah}:${event.ayah}`;
  const [reads, ms] = day[key] ?? [0, 0];
  day[key] = [reads + 1, ms + event.ms];
  days[event.localDay] = day;
  // YYYY-MM-DD sorts lexicographically; keep the newest ROLLUP_DAY_CAP days.
  const kept = Object.keys(days).sort().slice(-ROLLUP_DAY_CAP);
  writeJSON(ROLLUP_KEY, { v: 2, days: Object.fromEntries(kept.map((d) => [d, days[d]])) });
}

/** Trailing debounce so bursts of verse reads flush as one batch. */
const flush = trailingDebounce(() => void flushNow(), 4_000);

async function flushNow(): Promise<void> {
  if (!browser) return;
  const owner = currentOwner();
  if (owner === LOCAL_OWNER) return;
  absorbLocal(owner);
  const queued = readQueue(owner);
  if (queued.length === 0) return;
  const batch = queued.slice(0, BATCH_MAX);
  try {
    const res = await authClient.unsafeRequest<{ accepted: number }>("/user/analytics/v1/events", {
      method: "POST",
      body: { expectedUserId: Number(owner), events: batch },
    });
    if (res.ok || res.status === 400) {
      // A 400 rejects the whole batch (validated before any write): drop it so
      // one poison batch cannot block the queue forever. Offline / 401 / 403
      // keep the queue. Retries are safe — the server is idempotent by id.
      if (!res.ok) console.warn("[analytics] dropped a rejected event batch");
      const sent = new Set(batch.map((e) => e.id));
      const rest = readQueue(owner).filter((e) => !sent.has(e.id));
      writeQueue(owner, rest);
      if (rest.length > 0) flush.schedule();
    }
  } catch {
    // Network failure: the queue stays durable; the next emit retries.
  }
}

/** Sign-in: the device's anonymous events join the account queue (once). */
function absorbLocal(owner: string): void {
  const local = readQueue(LOCAL_OWNER);
  if (local.length === 0) return;
  const seen = new Set(readQueue(owner).map((e) => e.id));
  writeQueue(owner, [...readQueue(owner), ...local.filter((e) => !seen.has(e.id))]);
  removeJSON(queueKeyFor(LOCAL_OWNER));
  removeJSON(ROLLUP_KEY);
}

export interface ReadInput {
  readonly sessionId: string | null;
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
  readonly ms: number;
}

function emit(input: ReadInput): void {
  if (!browser) return;
  if (!Number.isInteger(input.surah) || input.surah < 1 || input.surah > 114) return;
  if (!Number.isInteger(input.ayah) || input.ayah < 1 || input.ayah > 286) return;
  const event: ReadingEvent = {
    id: newEntityId(),
    sessionId: input.sessionId,
    surah: input.surah,
    ayah: input.ayah,
    sourceId: input.sourceId !== null && SOURCE_ID_RE.test(input.sourceId) ? input.sourceId : null,
    ms: Math.max(0, Math.min(MS_MAX, Math.round(input.ms))),
    localDay: todayLocal(),
    at: new Date().toISOString(),
  };
  const owner = currentOwner();
  writeQueue(owner, [...readQueue(owner), event]);
  if (owner === LOCAL_OWNER) {
    recordLocal(event);
    return;
  }
  flush.schedule();
}

async function fetchSummary(): Promise<ReadingSummary | null> {
  if (!browser) return null;
  const owner = currentOwner();
  if (owner === LOCAL_OWNER) return summarizeRollup(decodeLocalRollup(readJSON(ROLLUP_KEY)));
  await flushNow();
  try {
    const res = await authClient.get("/user/analytics/v1/summary");
    return res.ok && res.data ? decodeSummary(res.data) : null;
  } catch {
    return null;
  }
}

if (browser) onPageHide(() => void flushNow());

export const readingAnalytics = { emit, flushNow, fetchSummary };
