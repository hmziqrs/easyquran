import { parseKey } from "#lib/data/quran.js";
import { dailyGlobalIndex } from "#lib/home/daily-ayah.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { readRangeText } from "#lib/server/quran-sqlite.js";

export interface DailyAyahView {
  readonly date: string;
  readonly surah: number;
  readonly ayah: number;
  readonly slug: string;
  readonly name: string;
  readonly text: string;
}

/**
 * The ayah for a UTC date, read server-side from the same reader DB the
 * translated routes use. Null when the verse or its text cannot be read, so the
 * page can say so instead of failing the request.
 */
export function loadDailyAyah(utcDate: string): DailyAyahView | null {
  const global = dailyGlobalIndex(utcDate);
  const key = QURAN_DATA.verseKeyAtGlobal(global);
  if (key === undefined) return null;
  const { num, n } = parseKey(key);
  const surah = QURAN_DATA.surahByNum(num);
  if (!surah) return null;
  try {
    const row = readRangeText(global, global).ayahs[0];
    if (!row || row.text.length === 0) return null;
    return {
      date: utcDate,
      surah: num,
      ayah: n,
      slug: surah.slug,
      name: surah.name,
      text: row.text,
    };
  } catch (error) {
    console.warn(`[daily-ayah] ayah ${key} unavailable for ${utcDate}`, error);
    return null;
  }
}
