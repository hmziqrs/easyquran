import { QURAN } from "#lib/config/site.js";
import { fetchJsonWithTimeout } from "#lib/quran/fetch.js";
import { unwrapEnvelope } from "#lib/quran/wire.js";
import { asNumber, asObject, asString } from "#lib/storage/index.js";

/**
 * Daily-ayah widget fetch (Feature 5.1 / D07): the backend's `/quran/random` is
 * deterministic per UTC date and serves Cache-Control to UTC midnight, so the
 * plain GET (no explicit date) is the canonical call — every visitor on a day
 * sees the same ayah and the browser caches it until the next UTC day.
 */

export interface DailyAyah {
  readonly surah: number;
  readonly ayah: number;
  readonly text: string;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unwrapped `/quran/random` JSON body; this decoder is its parse boundary.
function decodeDailyAyah(raw: unknown): DailyAyah | null {
  const body = asObject(unwrapEnvelope(raw));
  if (!body) return null;
  const ayah = asObject(body.ayah);
  const surah = ayah === undefined ? undefined : asNumber(ayah.surah, 1, 114);
  const num = ayah === undefined ? undefined : asNumber(ayah.ayah, 1, 286);
  const text = ayah === undefined ? undefined : asString(ayah.text);
  if (surah === undefined || num === undefined || text === undefined || text.length === 0)
    return null;
  return { surah, ayah: num, text };
}

/*
 * Mirrors `random_global` in rust/backend/api/src/modules/quran_v1/controller.rs
 * (K, C over the 6236-verse permutation, 1-based). Keep the two in lockstep: the
 * public page and the API must name the same verse for the same UTC date.
 */
const DAILY_K = 1103;
const DAILY_C = 4177;
const DAILY_VERSE_COUNT = 6236;
const DAY_MS = 86_400_000;

/** The UTC calendar date of `now` as `YYYY-MM-DD`: the key every visitor shares for a day. */
export function utcDateKey(now: Date): string {
  return now.toISOString().slice(0, 10);
}

/** Seconds from `now` to the next 00:00 UTC, at least 1. The cache horizon for a day's ayah. */
export function secondsUntilUtcMidnight(now: Date): number {
  const nextMidnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1);
  return Math.max(1, Math.ceil((nextMidnight - now.getTime()) / 1000));
}

/** 1-based global ayah index for a `YYYY-MM-DD` UTC date. Same verse for every caller that day. */
export function dailyGlobalIndex(utcDate: string): number {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(utcDate);
  if (!match) throw new RangeError(`daily ayah date must be YYYY-MM-DD: ${utcDate}`);
  const [, year, month, day] = match;
  const days = Math.round(
    (Date.UTC(Number(year), Number(month) - 1, Number(day)) - Date.UTC(1970, 0, 1)) / DAY_MS,
  );
  const slot =
    (((days * DAILY_K + DAILY_C) % DAILY_VERSE_COUNT) + DAILY_VERSE_COUNT) % DAILY_VERSE_COUNT;
  return slot + 1;
}

export async function fetchDailyAyah(signal?: AbortSignal): Promise<DailyAyah | null> {
  if (!QURAN.apiBase) return null;
  try {
    const body = await fetchJsonWithTimeout(`${QURAN.apiBase}/random`, {
      headers: { accept: "application/json" },
      signal,
    });
    return decodeDailyAyah(body);
  } catch {
    return null;
  }
}
