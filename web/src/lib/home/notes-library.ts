import type { NoteEntry } from "#lib/stores/annotations.svelte.js";

/** Library ordering. `verse` is mushaf order; the other two order by last edit. */
export type NoteSort = "verse" | "newest" | "oldest";

export const NOTE_SORTS: readonly NoteSort[] = ["verse", "newest", "oldest"];

/** Case-insensitive match on note text, `surah:ayah`, or the surah name. */
export function filterNotes(
  entries: readonly NoteEntry[],
  query: string,
  surahName: (surah: number) => string,
): readonly NoteEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  if (needle === "") return entries;
  return entries.filter((entry) => {
    if (entry.text.toLocaleLowerCase().includes(needle)) return true;
    if (`${entry.surah}:${entry.ayah}`.includes(needle)) return true;
    return surahName(entry.surah).toLocaleLowerCase().includes(needle);
  });
}

function byVerse(a: NoteEntry, b: NoteEntry): number {
  return a.surah - b.surah || a.ayah - b.ayah;
}

/**
 * Orders notes for the library. Notes without a timestamp (written before timestamps
 * existed) count as oldest. Ties keep mushaf order because the sort is stable.
 */
export function sortNotes(entries: readonly NoteEntry[], sort: NoteSort): readonly NoteEntry[] {
  const verseOrder = [...entries].sort(byVerse);
  if (sort === "verse") return verseOrder;
  const direction = sort === "newest" ? -1 : 1;
  return verseOrder.sort((a, b) => direction * ((a.updatedAt ?? 0) - (b.updatedAt ?? 0)));
}
