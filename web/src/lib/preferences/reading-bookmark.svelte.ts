import { asNumber, asObject, asString } from "#lib/storage/index.js";

import { preferences, READING_BOOKMARK_KEY, type PreferencesStore } from "./store.svelte";

/**
 * The page the reader chose to come back to. Separate from automatic resume: nothing
 * in the reader writes it except an explicit set, replace, clear or undo.
 */
export interface ReadingBookmark {
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
  readonly setAt: number;
}

export function currentReadingBookmark(store: PreferencesStore = preferences): ReadingBookmark | null {
  const value = asObject(store.get(READING_BOOKMARK_KEY));
  const surah = asNumber(value?.surah, 1, 114);
  const ayah = asNumber(value?.ayah, 1, 286);
  const setAt = asNumber(value?.setAt, 0, Number.MAX_SAFE_INTEGER);
  if (surah === undefined || ayah === undefined || setAt === undefined) return null;
  return { surah, ayah, sourceId: asString(value?.sourceId) ?? null, setAt };
}

/** Set or replace. Returns what it replaced, for Undo. */
export function setReadingBookmark(
  next: { readonly surah: number; readonly ayah: number; readonly sourceId: string | null },
  store: PreferencesStore = preferences,
): ReadingBookmark | null {
  const previous = currentReadingBookmark(store);
  store.set(READING_BOOKMARK_KEY, { ...next, setAt: Date.now() });
  return previous;
}

/** Undo puts the earlier value back with a newer stamp, or clears when there was none. */
export function restoreReadingBookmark(
  previous: ReadingBookmark | null,
  store: PreferencesStore = preferences,
): void {
  store.set(READING_BOOKMARK_KEY, previous === null ? null : { ...previous });
}
