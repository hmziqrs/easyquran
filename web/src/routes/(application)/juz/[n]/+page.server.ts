import { loadRangeData } from "#lib/server/quran-range.js";
import { rangeEntries, requireRangeIndex } from "#lib/server/reader-route-guards.js";

import type { PageServerLoad } from "./$types";

export const prerender = true;

export function entries() {
  return rangeEntries("juz");
}

export const load: PageServerLoad = ({ params }) =>
  loadRangeData("juz", requireRangeIndex("juz", params.n));
