import type { WordCoordinate, WordEntry, WordKey, WordSource } from "./types.ts";
import { wordKey } from "./types.ts";

const tables = new Map<string, Promise<ReadonlyMap<WordKey, WordEntry>>>();

/**
 * One load per (source, surah), shared by every verse on the page. A failed
 * load is dropped from the cache so the next open retries.
 */
export function loadWordTable(
  source: WordSource,
  surah: number,
): Promise<ReadonlyMap<WordKey, WordEntry>> {
  const cacheKey = `${source.id}:${surah}`;
  const cached = tables.get(cacheKey);
  if (cached !== undefined) return cached;
  const pending = source.load(surah);
  tables.set(cacheKey, pending);
  void pending.catch(() => {
    tables.delete(cacheKey);
  });
  return pending;
}

export function lookupWord(
  table: ReadonlyMap<WordKey, WordEntry>,
  coordinate: WordCoordinate,
): WordEntry | undefined {
  return table.get(wordKey(coordinate));
}

/** Test seam: forget every cached table. */
export function clearWordTables(): void {
  tables.clear();
}
