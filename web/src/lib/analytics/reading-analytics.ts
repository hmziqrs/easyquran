import { browser } from "$app/env";

import { authClient, isNumber, isString, isWireObject } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { todayLocal } from "#lib/quran/engagement-state.js";
import {
  asLiteral,
  asNumber,
  asObject,
  asString,
  onStorageKey,
  onPageHide,
  readJSON,
  removeJSON,
  trailingDebounce,
  writeJSON,
} from "#lib/storage/index.js";
import { CONSENT_STORAGE_KEY, consent, decodeConsent } from "#lib/stores/consent.svelte.js";

/**
 * First-party reading analytics (next-features Feature 6). Strictly opt-in and
 * OFF by default (§11.4): a per-ayah reading history is religious practice
 * (GDPR Art. 9 special-category data), so nothing is emitted, queued, or rolled
 * up locally unless `consent.readingAnalytics` is explicitly true — a flag
 * fully separate from the GA `analytics` consent.
 *
 * Transport is `AuthClient` (session cookie + `csrf-token`, never the sync
 * engine): signed-in events queue durably per owner under
 * `easyquran.analytics.queue.{owner}` and flush in batches to
 * `POST /user/analytics/v1/events`; anonymous reading falls back to the capped
 * device-local rollup `easyquran.analytics.local` (server summary takes over
 * once signed in). Queued events follow §9.5 on logout — the owner's partition
 * is kept, never destroyed — while opting out drops every queued event.
 */

const ROLLUP_KEY = "easyquran.analytics.local";
const QUEUE_KEY_PREFIX = "easyquran.analytics.queue.";
/** Server caps one batch at 100 events (user_analytics_v1 validator). */
const BATCH_MAX = 100;
/** Durable queue bound: ~5 full batches before the oldest events fall off. */
const QUEUE_CAP = 500;
/** Local rollup keeps 30 device-local days (EC-36: client-supplied days). */
const ROLLUP_DAY_CAP = 30;
/** Bound a single day's verse map so the rollup key cannot grow unbounded. */
const ROLLUP_VERSES_PER_DAY = 500;
/** Catalogue-id charset (§9.1), mirrored from the server validator. */
const SOURCE_ID_MAX = 64;

export const READING_EVENT_KINDS = ["ayah.read", "ayah.open"] as const;
export type ReadingEventKind = (typeof READING_EVENT_KINDS)[number];

export interface ReadingEvent {
  readonly kind: ReadingEventKind;
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
  readonly seconds: number | null;
  readonly localDay: string;
  readonly at: string;
}

/** `easyquran.analytics.local` v1: days[localDay][surah][ayah] = read count. */
export interface LocalRollup {
  readonly v: 1;
  readonly days: Record<string, Record<string, Record<string, number>>>;
}

export interface ReadingSummary {
  readonly totals: { readonly events: number; readonly seconds: number };
  readonly topSurahs: readonly { readonly surah: number; readonly reads: number }[];
  readonly topAyahs: readonly {
    readonly surah: number;
    readonly ayah: number;
    readonly reads: number;
  }[];
  readonly days: readonly {
    readonly localDay: string;
    readonly ayahsRead: number;
    readonly seconds: number;
  }[];
}

function emptyRollup(): LocalRollup {
  return { v: 1, days: {} };
}

function queueKeyFor(owner: string): string {
  return `${QUEUE_KEY_PREFIX}${owner}`;
}

/** Signed-in owner id, else null (anonymous reading only ever rolls up locally). */
function currentOwner(): string | null {
  if (authState.status !== "authenticated" || authState.user === null) return null;
  return String(authState.user.id);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed localStorage rollup; this decoder is its boundary parser.
export function decodeLocalRollup(raw: unknown): LocalRollup {
  const stored = asObject(raw);
  if (!stored || stored.v !== 1) return emptyRollup();
  const daysRaw = asObject(stored.days);
  const days: Record<string, Record<string, Record<string, number>>> = {};
  if (daysRaw) {
    for (const [day, surahsRaw] of Object.entries(daysRaw)) {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(day)) continue;
      const surahs = asObject(surahsRaw);
      if (!surahs) continue;
      const dayMap: Record<string, Record<string, number>> = {};
      for (const [surah, ayahsRaw] of Object.entries(surahs)) {
        if (!/^\d{1,3}$/.test(surah)) continue;
        const ayahs = asObject(ayahsRaw);
        if (!ayahs) continue;
        const ayahMap: Record<string, number> = {};
        for (const [ayah, countRaw] of Object.entries(ayahs)) {
          if (!/^\d{1,3}$/.test(ayah)) continue;
          const count = asNumber(countRaw, 1, 1_000_000);
          if (count !== undefined) ayahMap[ayah] = count;
        }
        dayMap[surah] = ayahMap;
      }
      days[day] = dayMap;
    }
  }
  return { v: 1, days };
}

/** Same shape as the server's `/summary` response, computed from the local rollup. */
export function summarizeRollup(rollup: LocalRollup): ReadingSummary {
  const perSurah = new Map<string, number>();
  const perAyah = new Map<string, number>();
  const perDay = new Map<string, number>();
  let events = 0;
  for (const [day, surahs] of Object.entries(rollup.days)) {
    let dayReads = 0;
    for (const [surah, ayahs] of Object.entries(surahs)) {
      for (const [ayah, count] of Object.entries(ayahs)) {
        events += count;
        dayReads += count;
        perSurah.set(surah, (perSurah.get(surah) ?? 0) + count);
        perAyah.set(`${surah}:${ayah}`, (perAyah.get(`${surah}:${ayah}`) ?? 0) + count);
      }
    }
    perDay.set(day, dayReads);
  }
  return {
    totals: { events, seconds: 0 },
    topSurahs: [...perSurah.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 5)
      .map(([surah, reads]) => ({ surah: Number(surah), reads })),
    topAyahs: [...perAyah.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 5)
      .map(([key, reads]) => {
        const [surah, ayah] = key.split(":");
        return { surah: Number(surah), ayah: Number(ayah), reads };
      }),
    days: [...perDay.entries()]
      .sort(([a], [b]) => b.localeCompare(a))
      .slice(0, 30)
      .map(([localDay, ayahsRead]) => ({ localDay, ayahsRead, seconds: 0 })),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed fetch body of GET /user/analytics/v1/summary; this decoder is its boundary parser.
export function decodeSummary(raw: unknown): ReadingSummary | null {
  if (!isWireObject(raw)) return null;
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- value is one unparsed summary sub-list inside that body; this inner decoder is part of the same boundary.
  const list = (value: unknown): { surah: number; ayah?: number; reads: number }[] => {
    if (!Array.isArray(value)) return [];
    return value.flatMap((item) => {
      if (!isWireObject(item) || !isNumber(item.surah) || !isNumber(item.reads)) return [];
      const ayah = isNumber(item.ayah) ? item.ayah : undefined;
      return [{ surah: item.surah, ayah, reads: item.reads }];
    });
  };
  const totals = isWireObject(raw.totals)
    ? {
        events: isNumber(raw.totals.events) ? raw.totals.events : 0,
        seconds: isNumber(raw.totals.seconds) ? raw.totals.seconds : 0,
      }
    : { events: 0, seconds: 0 };
  return {
    totals,
    topSurahs: list(raw.topSurahs).map(({ surah, reads }) => ({ surah, reads })),
    topAyahs: list(raw.topAyahs).flatMap(({ surah, ayah, reads }) =>
      ayah === undefined ? [] : [{ surah, ayah, reads }],
    ),
    days: list2Days(raw.days),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- value is the unparsed days sub-list of the summary body; same boundary as decodeSummary.
function list2Days(value: unknown): ReadingSummary["days"] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!isWireObject(item) || !isString(item.localDay)) return [];
    return [
      {
        localDay: item.localDay,
        ayahsRead: isNumber(item.ayahsRead) ? item.ayahsRead : 0,
        seconds: isNumber(item.seconds) ? item.seconds : 0,
      },
    ];
  });
}

function validSourceId(id: string): boolean {
  if (id.length > SOURCE_ID_MAX) return false;
  for (const c of id) {
    if (!(c >= "a" && c <= "z") && !(c >= "0" && c <= "9") && c !== "." && c !== "_" && c !== "-") {
      return false;
    }
  }
  return true;
}

function readQueue(owner: string): ReadingEvent[] {
  const stored = asObject(readJSON(queueKeyFor(owner)));
  const eventsRaw = Array.isArray(stored?.events) ? stored.events : [];
  return eventsRaw.flatMap((raw) => {
    const o = asObject(raw);
    if (!o) return [];
    const kind = asLiteral<ReadingEventKind>(o.kind, READING_EVENT_KINDS);
    const surah = asNumber(o.surah, 1, 114);
    const ayah = asNumber(o.ayah, 1, 286);
    const localDay = asString(o.localDay);
    const at = asString(o.at);
    const sourceId = asString(o.sourceId);
    const seconds = asNumber(o.seconds, 0, 3600);
    if (
      kind === undefined ||
      surah === undefined ||
      ayah === undefined ||
      localDay === undefined ||
      at === undefined
    ) {
      return [];
    }
    return [
      {
        kind,
        surah,
        ayah,
        sourceId: sourceId !== undefined && sourceId.length > 0 ? sourceId : null,
        seconds: seconds !== undefined ? seconds : null,
        localDay,
        at,
      },
    ];
  });
}

function writeQueue(owner: string, events: readonly ReadingEvent[]): void {
  if (events.length === 0) {
    removeJSON(queueKeyFor(owner));
    return;
  }
  // FIFO trim: when the cap is hit the OLDEST events fall off, never the new one.
  writeJSON(queueKeyFor(owner), { v: 1, events: events.slice(-QUEUE_CAP) });
}

function recordLocal(event: ReadingEvent): void {
  const rollup = decodeLocalRollup(readJSON(ROLLUP_KEY));
  const days = { ...rollup.days };
  const day = days[event.localDay] ?? {};
  const surah = day[String(event.surah)] ?? {};
  // Bound a single day's verse map: past the cap only already-counted verses grow.
  if (
    Object.keys(surah).length < ROLLUP_VERSES_PER_DAY ||
    surah[String(event.ayah)] !== undefined
  ) {
    surah[String(event.ayah)] = (surah[String(event.ayah)] ?? 0) + 1;
  }
  day[String(event.surah)] = surah;
  days[event.localDay] = day;
  // YYYY-MM-DD sorts lexicographically; keep the newest ROLLUP_DAY_CAP days.
  const kept = Object.keys(days).sort().slice(-ROLLUP_DAY_CAP);
  writeJSON(ROLLUP_KEY, { v: 1, days: Object.fromEntries(kept.map((d) => [d, days[d]])) });
}

/** Trailing debounce so bursts of verse writes flush as one batch. */
const flush = trailingDebounce(() => void flushNow(), 4_000);

async function flushNow(): Promise<void> {
  if (!browser || !consent.readingAnalytics) return;
  const owner = currentOwner();
  if (owner === null) return;
  const queued = readQueue(owner);
  if (queued.length === 0) return;
  const batch = queued.slice(0, BATCH_MAX);
  try {
    const res = await authClient.unsafeRequest<{ accepted: number }>("/user/analytics/v1/events", {
      method: "POST",
      body: { expectedUserId: Number(owner), events: batch },
    });
    if (res.ok || res.status === 400) {
      // A 400 rejects the whole batch (all-mutations-validated-first): drop it
      // so one poison batch cannot head-of-line-block the queue forever
      // (EC-29's rule — never an offline/401/403 round, which keeps the queue).
      if (!res.ok) console.warn("[analytics] dropped a rejected event batch");
      writeQueue(owner, queued.slice(batch.length));
      if (queued.length > batch.length) flush.schedule();
    }
  } catch {
    // Network failure: the queue stays durable; the next emit retries.
  }
}

/**
 * Opt-out is a device-global revocation, so it removes the rollup plus EVERY
 * owner's queued batch (`easyquran.analytics.queue.*`), not just the account
 * currently signed in — a signed-out opt-out must also delete another user's
 * left-behind partition, matching the copy "Turning this off deletes anything
 * still queued". Storage access itself can throw when site data is blocked, so
 * the enumeration sits inside a try like the safe-storage helpers.
 */
function dropAllAnalyticsData(): void {
  removeJSON(ROLLUP_KEY);
  try {
    for (const key of Object.keys(window.localStorage)) {
      if (key.startsWith(QUEUE_KEY_PREFIX)) removeJSON(key);
    }
  } catch {
    // Blocked storage: nothing queued was readable either.
  }
}

function emit(event: Omit<ReadingEvent, "localDay" | "at"> & { readonly localDay?: string }): void {
  if (!browser || !consent.readingAnalytics) return;
  if (!Number.isInteger(event.surah) || event.surah < 1 || event.surah > 114) return;
  if (!Number.isInteger(event.ayah) || event.ayah < 1 || event.ayah > 286) return;
  if (event.sourceId !== null && !validSourceId(event.sourceId)) return;
  const full: ReadingEvent = {
    ...event,
    seconds:
      event.seconds !== null && event.seconds >= 0 && event.seconds <= 3600 ? event.seconds : null,
    localDay: event.localDay ?? todayLocal(),
    at: new Date().toISOString(),
  };
  const owner = currentOwner();
  if (owner === null) {
    recordLocal(full);
    return;
  }
  writeQueue(owner, [...readQueue(owner), full]);
  flush.schedule();
}

async function fetchSummary(): Promise<ReadingSummary | null> {
  if (!browser || !consent.readingAnalytics) return null;
  const owner = currentOwner();
  if (owner === null) return summarizeRollup(decodeLocalRollup(readJSON(ROLLUP_KEY)));
  try {
    const res = await authClient.get("/user/analytics/v1/summary");
    return res.ok && res.data ? decodeSummary(res.data) : null;
  } catch {
    return null;
  }
}

if (browser) {
  // Opt-out drops every queued event immediately (§11.4) — same tab via the
  // consent event, other tabs via the storage mirror.
  window.addEventListener("easyquran:consent", (e) => {
    // SAFETY: consent.set() dispatches CustomEvent<Partial<ConsentFlags>> on this exact channel; the assertion only recovers that known payload type from the generic Event.
    const patch = (e as CustomEvent<Partial<{ readingAnalytics: boolean }>>).detail;
    if (patch && patch.readingAnalytics === false) dropAllAnalyticsData();
  });
  onStorageKey(CONSENT_STORAGE_KEY, () => {
    // Decode the fresh stored value, never the in-memory flag: the consent
    // store in THIS tab may not have re-read the other tab's write yet, and a
    // stale `true` would keep this tab's queues alive (listener registration
    // order is not guaranteed).
    if (!decodeConsent(readJSON(CONSENT_STORAGE_KEY)).readingAnalytics) dropAllAnalyticsData();
  });
  onPageHide(() => void flushNow());
}

export const readingAnalytics = { emit, flushNow, fetchSummary };
