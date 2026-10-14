import { toast } from "svelte-sonner";

import { getBookmarksCopy } from "#lib/i18n/bookmarks-copy.js";

import { bookmarks } from "./store.svelte";

/**
 * One-tap bookmarking with feedback instead of dialogs: a tap saves to the
 * last-used folder and says where ("Saved to Hifz · Change"); a second tap
 * removes it from every folder with an Undo. "Change" opens the folder picker.
 */

export interface SaveTarget {
  readonly surah: number;
  readonly ayah: number;
  readonly sourceId: string | null;
}

class SaveToPicker {
  #target = $state.raw<SaveTarget | null>(null);

  get target(): SaveTarget | null {
    return this.#target;
  }

  open(target: SaveTarget): void {
    this.#target = target;
  }

  close(): void {
    this.#target = null;
  }
}

export const saveToPicker = new SaveToPicker();

/** One toast slot: rapid taps replace each other instead of stacking. */
const TOAST_ID = "bookmark-feedback";

export function toggleBookmark(target: SaveTarget): void {
  const copy = getBookmarksCopy();
  const result = bookmarks.toggle(target.surah, target.ayah, target.sourceId);
  if (result.kind === "added") {
    toast(copy.savedTo(result.folder.name), {
      id: TOAST_ID,
      action: { label: copy.change, onClick: () => saveToPicker.open(target) },
    });
  } else if (result.kind === "removed") {
    toast(copy.removed, {
      id: TOAST_ID,
      action: { label: copy.undo, onClick: () => bookmarks.restore(result.bookmarks) },
    });
  }
}
