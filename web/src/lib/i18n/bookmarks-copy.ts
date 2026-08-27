import type { UiDirection, UiLocale } from "$lib/i18n/locales";
import { uiDirection } from "$lib/i18n/locales";
import {
  reader_bookmarks_ayah,
  reader_bookmarks_cancel,
  reader_bookmarks_create_folder,
  reader_bookmarks_delete_folder,
  reader_bookmarks_delete_folder_confirm,
  reader_bookmarks_empty,
  reader_bookmarks_folder_empty,
  reader_bookmarks_folders_heading,
  reader_bookmarks_local_only,
  reader_bookmarks_move_label,
  reader_bookmarks_move_to,
  reader_bookmarks_new_folder_label,
  reader_bookmarks_new_folder_placeholder,
  reader_bookmarks_offline,
  reader_bookmarks_open,
  reader_bookmarks_pending,
  reader_bookmarks_remove,
  reader_bookmarks_remove_label,
  reader_bookmarks_rename,
  reader_bookmarks_rename_label,
  reader_bookmarks_save,
  reader_bookmarks_sign_in,
  reader_bookmarks_sign_in_note,
  reader_bookmarks_sync_error,
  reader_bookmarks_synced,
  reader_bookmarks_title,
  reader_bookmarks_uncategorized,
} from "$lib/i18n/m/reader";
import { getLocale } from "$lib/paraglide/runtime.js";

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
  readonly remove: string;
  readonly removeLabel: string;
  readonly open: string;
  readonly moveTo: string;
  readonly moveLabel: (reference: string) => string;
  readonly ayah: (ayah: number) => string;
  readonly signInNote: string;
  readonly signIn: string;
  readonly offline: string;
  readonly pending: (count: number) => string;
  readonly synced: string;
  readonly syncError: string;
  readonly localOnly: string;
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
    remove: noArgs(reader_bookmarks_remove),
    removeLabel: noArgs(reader_bookmarks_remove_label),
    open: noArgs(reader_bookmarks_open),
    moveTo: noArgs(reader_bookmarks_move_to),
    moveLabel: (reference) => reader_bookmarks_move_label({ reference }, options),
    ayah: (ayah) => reader_bookmarks_ayah({ ayah }, options),
    signInNote: noArgs(reader_bookmarks_sign_in_note),
    signIn: noArgs(reader_bookmarks_sign_in),
    offline: noArgs(reader_bookmarks_offline),
    pending: (count) => reader_bookmarks_pending({ count }, options),
    synced: noArgs(reader_bookmarks_synced),
    syncError: noArgs(reader_bookmarks_sync_error),
    localOnly: noArgs(reader_bookmarks_local_only),
  };
}
