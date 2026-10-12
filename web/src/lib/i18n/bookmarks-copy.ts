import type { UiDirection, UiLocale } from "#lib/i18n/locales.js";
import { uiDirection } from "#lib/i18n/locales.js";
import {
  reader_bookmarks_ayah,
  reader_bookmarks_cancel,
  reader_bookmarks_clear_selection,
  reader_bookmarks_create_folder,
  reader_bookmarks_delete_folder,
  reader_bookmarks_delete_folder_confirm,
  reader_bookmarks_empty,
  reader_bookmarks_folder_empty,
  reader_bookmarks_folder_kind_daily,
  reader_bookmarks_folder_kind_generic,
  reader_bookmarks_folder_kind_label,
  reader_bookmarks_folder_kind_learning,
  reader_bookmarks_folder_progress,
  reader_bookmarks_folders_heading,
  reader_bookmarks_local_only,
  reader_bookmarks_loading,
  reader_bookmarks_move_label,
  reader_bookmarks_move_selected_to,
  reader_bookmarks_move_to,
  reader_bookmarks_move_down,
  reader_bookmarks_move_up,
  reader_bookmarks_new_folder_label,
  reader_bookmarks_new_folder_placeholder,
  reader_bookmarks_note_label,
  reader_bookmarks_offline,
  reader_bookmarks_pending,
  reader_bookmarks_pending_one,
  reader_bookmarks_remove,
  reader_bookmarks_remove_label,
  reader_bookmarks_remove_selected,
  reader_bookmarks_removed,
  reader_bookmarks_rename,
  reader_bookmarks_rename_label,
  reader_bookmarks_save,
  reader_bookmarks_saved,
  reader_bookmarks_select_label,
  reader_bookmarks_selected,
  reader_bookmarks_sign_in,
  reader_bookmarks_sign_in_note,
  reader_bookmarks_sync_error,
  reader_bookmarks_synced,
  reader_bookmarks_title,
  reader_bookmarks_uncategorized,
  reader_bookmarks_undo,
} from "#lib/i18n/m/reader.js";

import type { BookmarkFolderKind } from "#lib/bookmarks/schema.js";
import { getLocale } from "#lib/paraglide/runtime.js";

/** Kind → message fn; a lookup table, not a ternary chain (AGENTS.MD rule). */
const FOLDER_KIND_MESSAGES = {
  generic: reader_bookmarks_folder_kind_generic,
  daily: reader_bookmarks_folder_kind_daily,
  learning: reader_bookmarks_folder_kind_learning,
} as const satisfies Record<
  BookmarkFolderKind,
  (inputs?: undefined, options?: { locale?: UiLocale }) => string
>;

export interface BookmarksCopy {
  readonly locale: UiLocale;
  readonly direction: UiDirection;
  readonly title: string;
  readonly empty: string;
  readonly folderEmpty: string;
  readonly unfiled: string;
  readonly foldersHeading: string;
  readonly newFolderLabel: string;
  readonly newFolderPlaceholder: string;
  readonly createFolder: string;
  readonly rename: string;
  readonly renameLabel: string;
  readonly save: string;
  readonly cancel: string;
  readonly deleteFolder: string;
  readonly deleteFolderConfirm: (name: string) => string;
  /** Study-set label (§11.7: name-only — never conflated with reading sessions). */
  readonly folderKind: (kind: BookmarkFolderKind) => string;
  readonly folderKindLabel: string;
  readonly moveTo: string;
  readonly moveUpLabel: (name: string) => string;
  readonly moveDownLabel: (name: string) => string;
  /** Bulk selection toolbar (Feature 2.3, P05). */
  readonly selected: (count: number) => string;
  readonly moveSelectedTo: string;
  readonly removeSelected: string;
  readonly clearSelection: string;
  readonly selectLabel: (reference: string) => string;
  /** Saved-date line (BM-02); `when` is a locale-formatted relative day. */
  readonly saved: (when: string) => string;
  /** Daily-folder progress caption (Feature 2.3). */
  readonly folderProgress: (done: number, total: number) => string;
  readonly noteLabel: string;
  readonly remove: string;
  readonly removeLabel: string;
  readonly removed: string;
  readonly undo: string;
  readonly moveLabel: (reference: string) => string;
  readonly ayah: (ayah: number) => string;
  readonly signInNote: string;
  readonly signIn: string;
  readonly offline: string;
  readonly pending: (count: number) => string;
  readonly synced: string;
  readonly syncError: string;
  readonly localOnly: string;
  readonly loading: string;
}

/**
 * Bookmarks page copy. Keys live in the `reader` namespace (reader_bookmarks_*
 * prefix): the page mounts inside the reader app shell, which already downloads
 * that namespace, so no new download boundary is warranted. See
 * docs/quran-system.md (Part 2, Message chunking).
 */
// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getBookmarksCopy(locale: UiLocale = getLocale() as UiLocale): BookmarksCopy {
  const options = { locale };
  const noArgs = (
    message: (inputs?: undefined, options?: { locale?: UiLocale }) => string,
  ): string => message(undefined, options);
  return {
    locale,
    direction: uiDirection(locale),
    title: noArgs(reader_bookmarks_title),
    empty: noArgs(reader_bookmarks_empty),
    folderEmpty: noArgs(reader_bookmarks_folder_empty),
    unfiled: noArgs(reader_bookmarks_uncategorized),
    foldersHeading: noArgs(reader_bookmarks_folders_heading),
    newFolderLabel: noArgs(reader_bookmarks_new_folder_label),
    newFolderPlaceholder: noArgs(reader_bookmarks_new_folder_placeholder),
    createFolder: noArgs(reader_bookmarks_create_folder),
    rename: noArgs(reader_bookmarks_rename),
    renameLabel: noArgs(reader_bookmarks_rename_label),
    save: noArgs(reader_bookmarks_save),
    cancel: noArgs(reader_bookmarks_cancel),
    deleteFolder: noArgs(reader_bookmarks_delete_folder),
    deleteFolderConfirm: (name) => reader_bookmarks_delete_folder_confirm({ name }, options),
    folderKind: (kind) => noArgs(FOLDER_KIND_MESSAGES[kind]),
    folderKindLabel: noArgs(reader_bookmarks_folder_kind_label),
    moveTo: noArgs(reader_bookmarks_move_to),
    moveUpLabel: (name) => reader_bookmarks_move_up({ name }, options),
    moveDownLabel: (name) => reader_bookmarks_move_down({ name }, options),
    selected: (count) => reader_bookmarks_selected({ count }, options),
    moveSelectedTo: noArgs(reader_bookmarks_move_selected_to),
    removeSelected: noArgs(reader_bookmarks_remove_selected),
    clearSelection: noArgs(reader_bookmarks_clear_selection),
    selectLabel: (reference) => reader_bookmarks_select_label({ reference }, options),
    saved: (when) => reader_bookmarks_saved({ when }, options),
    folderProgress: (done, total) => reader_bookmarks_folder_progress({ done, total }, options),
    noteLabel: noArgs(reader_bookmarks_note_label),
    remove: noArgs(reader_bookmarks_remove),
    removeLabel: noArgs(reader_bookmarks_remove_label),
    removed: noArgs(reader_bookmarks_removed),
    undo: noArgs(reader_bookmarks_undo),
    moveLabel: (reference) => reader_bookmarks_move_label({ reference }, options),
    ayah: (ayah) => reader_bookmarks_ayah({ ayah }, options),
    signInNote: noArgs(reader_bookmarks_sign_in_note),
    signIn: noArgs(reader_bookmarks_sign_in),
    offline: noArgs(reader_bookmarks_offline),
    // Singular/plural: en has no dual; ar folds 0/2+ into the plural form
    // ("{count} من التغييرات…") and uses the explicit singular for exactly 1.
    pending: (count) =>
      count === 1
        ? noArgs(reader_bookmarks_pending_one)
        : reader_bookmarks_pending({ count }, options),
    synced: noArgs(reader_bookmarks_synced),
    syncError: noArgs(reader_bookmarks_sync_error),
    localOnly: noArgs(reader_bookmarks_local_only),
    loading: noArgs(reader_bookmarks_loading),
  };
}
