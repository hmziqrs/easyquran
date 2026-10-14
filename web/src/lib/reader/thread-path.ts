import { hizbOfPage, juzOfPage, pageOfGlobal } from "#lib/data/mushaf-divisions.js";
import type { QuranData } from "#lib/data/quran-data.js";
import {
  globalPagePathFor,
  hizbPathFor,
  juzPathFor,
  snippetPathFor,
  surahAyahPathFor,
  type SurahRouteContext,
} from "#lib/data/quran.js";
import type { ReadingThread } from "#lib/sessions/schema.js";

/**
 * Reader path that continues a thread in the reader it was read in: page /
 * juz / hizb threads reopen the division holding the cursor, snippet threads
 * their snippet, everything else the surah at the cursor ayah.
 */
export function threadPath(
  thread: ReadingThread,
  ctx: SurahRouteContext,
  quranData: QuranData,
): `/${string}` | null {
  const [kind, ref] = thread.scope.split(":");
  const global = quranData.globalIndexOf(thread.surah, thread.ayah);
  if (kind === "snippet" && ref !== undefined) return snippetPathFor(ctx, ref);
  if (global !== undefined && kind === "page") {
    return globalPagePathFor(ctx, pageOfGlobal(quranData, global));
  }
  if (global !== undefined && kind === "juz") return juzPathFor(ctx, juzOfPage(quranData, global));
  if (global !== undefined && kind === "hizb") {
    return hizbPathFor(ctx, hizbOfPage(quranData, global));
  }
  const surah = quranData.surahByNum(thread.surah);
  if (!surah || !quranData.surahLocalPageForAyah(thread.surah, thread.ayah)) return null;
  return surahAyahPathFor(ctx, surah, thread.ayah);
}
