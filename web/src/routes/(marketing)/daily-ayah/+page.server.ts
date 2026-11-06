import { secondsUntilUtcMidnight, utcDateKey } from "#lib/home/daily-ayah.js";
import { loadDailyAyah } from "#lib/server/daily-ayah.js";

import type { PageServerLoad } from "./$types";

/**
 * Server-rendered per request, not prerendered: a prerendered page would freeze
 * one day's verse at build time. The verse is keyed on the UTC date, so every
 * visitor on a day gets the same HTML, cached until the next 00:00 UTC.
 */
export const prerender = false;

export const load: PageServerLoad = ({ setHeaders }) => {
  const now = new Date();
  const date = utcDateKey(now);
  const daily = loadDailyAyah(date);
  // A missing verse is not cached for the day: the next request retries the read.
  setHeaders({
    "cache-control": daily ? `public, max-age=${secondsUntilUtcMidnight(now)}` : "no-store",
  });
  return { daily, date };
};
