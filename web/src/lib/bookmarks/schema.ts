import type { VerseKey } from "#lib/data/quran.js";
import { asArray, asNumber, asObject, asString } from "#lib/storage/index.js";
import { uuidV5 } from "#lib/sync/ids.js";

/**
 * Folder-based bookmarks (docs/reading-model.md §3). Every bookmark lives in a
 * folder; every owner has a default folder whose id is derived (UUIDv5), so
 * offline devices converge on one default instead of minting duplicates. A
 * verse may sit in many folders. Wire JSON and the device-local envelope both
 * decode defensively here.
 */

export const SURAH_MIN = 1;
export const SURAH_MAX = 114;
/** Longest surah (Al-Baqarah). */
export const AYAH_MAX = 286;
export const FOLDER_NAME_MAX = 100;
/** Seed name of every default folder (the user may rename it). */
export const DEFAULT_FOLDER_NAME = "Bookmarks";

/**
 * UUIDv5 namespace for default-folder ids — byte-for-byte the server's
 * `DEFAULT_FOLDER_NAMESPACE` (bookmark_folder/actions.rs).
 */
const DEFAULT_FOLDER_NAMESPACE = "6f0e4c1a-2b7d-4e59-9a83-5c1d2e3f4a5b";

/** Default folder id: `user:{id}` for an account, `local` for the device owner. */
export function defaultFolderIdFor(owner: number | "local"): string {
  return uuidV5(DEFAULT_FOLDER_NAMESPACE, owner === "local" ? "local" : `user:${owner}`);
}

export interface BookmarkFolder {
  readonly id: string;
  readonly name: string;
  readonly isDefault: boolean;
  /** Manual order (LWW integer); null = unpositioned, sorts after positioned. */
  readonly position: number | null;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface Bookmark {
  readonly id: string;
  readonly folderId: string;
  readonly surah: number;
  readonly ayah: number;
  readonly createdAt: string;
  readonly updatedAt: string;
  /** Saved translation context: null → consumers fall back to Arabic. */
  readonly sourceId: string | null;
  /** Range end: null = single ayah. */
  readonly endSurah: number | null;
  readonly endAyah: number | null;
}

export type BookmarksMutation =
  | {
      readonly kind: "bookmark.upsert";
      readonly id: string;
      readonly folderId: string;
      readonly surah: number;
      readonly ayah: number;
      readonly endSurah: number | null;
      readonly endAyah: number | null;
      readonly sourceId: string | null;
      readonly updatedAt: string;
    }
  | { readonly kind: "bookmark.delete"; readonly id: string; readonly updatedAt: string }
  | {
      readonly kind: "folder.upsert";
      readonly id: string;
      readonly name: string;
      readonly position: number | null;
      readonly updatedAt: string;
    }
  | { readonly kind: "folder.delete"; readonly id: string; readonly updatedAt: string };

export interface BookmarksSnapshot {
  readonly folders: readonly BookmarkFolder[];
  readonly bookmarks: readonly Bookmark[];
}

export interface BookmarksSyncEnvelope extends BookmarksSnapshot {
  readonly applied: number;
  readonly skipped: number;
}

export function verseKeyOf(surah: number, ayah: number): VerseKey {
  return `${surah}:${ayah}`;
}

/** Strict parse of a VerseKey; null for anything not `${digits}:${digits}`. */
export function parseVerseKey(key: string): { surah: number; ayah: number } | null {
  const match = /^(\d+):(\d+)$/u.exec(key);
  if (!match) return null;
  return { surah: Number(match[1]), ayah: Number(match[2]) };
}

/** True when the row covers exactly one ayah. Per-verse marks match single rows only. */
export function isSingleVerseBookmark(
  bookmark: Pick<Bookmark, "surah" | "ayah" | "endSurah" | "endAyah">,
): boolean {
  return (
    (bookmark.endSurah ?? bookmark.surah) === bookmark.surah &&
    (bookmark.endAyah ?? bookmark.ayah) === bookmark.ayah
  );
}

/** Live key within a folder: start verse plus range end. */
export function rangeKeyOf(
  bookmark: Pick<Bookmark, "surah" | "ayah" | "endSurah" | "endAyah">,
): string {
  return `${verseKeyOf(bookmark.surah, bookmark.ayah)}-${verseKeyOf(
    bookmark.endSurah ?? bookmark.surah,
    bookmark.endAyah ?? bookmark.ayah,
  )}`;
}

/** Server-identical live key: folder + range. */
export function liveKeyOf(
  bookmark: Pick<Bookmark, "folderId" | "surah" | "ayah" | "endSurah" | "endAyah">,
): string {
  return `${bookmark.folderId}/${rangeKeyOf(bookmark)}`;
}

/** Default first, then manual position (unpositioned last), then name. */
export function compareFolders(a: BookmarkFolder, b: BookmarkFolder): number {
  if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1;
  const positions =
    (a.position ?? Number.MAX_SAFE_INTEGER) - (b.position ?? Number.MAX_SAFE_INTEGER);
  if (positions !== 0) return positions;
  return a.name.localeCompare(b.name);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed timestamp field; this decoder is its boundary parser.
function decodeTimestamp(raw: unknown): string | undefined {
  const value = asString(raw);
  return value !== undefined && value.length > 0 ? value : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed folder row (wire or storage); this decoder is its boundary parser.
export function decodeBookmarkFolder(raw: unknown): BookmarkFolder | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const name = asString(obj.name);
  if (id === undefined || id.length !== 36) return null;
  if (name === undefined || name.length === 0 || name.length > FOLDER_NAME_MAX) return null;
  const createdAt = decodeTimestamp(obj.createdAt);
  const updatedAt = decodeTimestamp(obj.updatedAt);
  if (createdAt === undefined || updatedAt === undefined) return null;
  return {
    id,
    name,
    isDefault: obj.isDefault === true,
    position: asNumber(obj.position, -1_000_000, 1_000_000) ?? null,
    createdAt,
    updatedAt,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed bookmark row (wire or storage); this decoder is its boundary parser.
export function decodeBookmark(raw: unknown): Bookmark | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const folderId = asString(obj.folderId);
  if (id === undefined || id.length !== 36 || folderId === undefined || folderId.length !== 36) {
    return null;
  }
  const surah = asNumber(obj.surah, SURAH_MIN, SURAH_MAX);
  const ayah = asNumber(obj.ayah, 1, AYAH_MAX);
  if (
    surah === undefined ||
    ayah === undefined ||
    !Number.isSafeInteger(surah) ||
    !Number.isSafeInteger(ayah)
  ) {
    return null;
  }
  const createdAt = decodeTimestamp(obj.createdAt);
  const updatedAt = decodeTimestamp(obj.updatedAt);
  if (createdAt === undefined || updatedAt === undefined) return null;
  // Range ends travel together or not at all; exactly one is a corrupt row.
  const endSurah = asNumber(obj.endSurah, SURAH_MIN, SURAH_MAX);
  const endAyah = asNumber(obj.endAyah, 1, AYAH_MAX);
  if ((endSurah === undefined) !== (endAyah === undefined)) return null;
  const sourceId = asString(obj.sourceId);
  return {
    id,
    folderId,
    surah,
    ayah,
    createdAt,
    updatedAt,
    sourceId: sourceId !== undefined && sourceId.length > 0 ? sourceId : null,
    endSurah: endSurah ?? null,
    endAyah: endAyah ?? null,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is a folders + bookmarks pair (wire snapshot or device envelope); this decoder is its boundary parser.
export function decodeBookmarksSnapshot(raw: unknown): BookmarksSnapshot | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.folders) || !Array.isArray(obj.bookmarks)) return null;
  return {
    folders: asArray(obj.folders, (item) => decodeBookmarkFolder(item) ?? undefined),
    bookmarks: asArray(obj.bookmarks, (item) => decodeBookmark(item) ?? undefined),
  };
}

/** Decode the `{ applied, skipped, folders, bookmarks }` sync response. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed JSON body of POST /bookmark/v1/sync; this decoder is its boundary parser.
export function decodeBookmarksEnvelope(raw: unknown): BookmarksSyncEnvelope | null {
  const snapshot = decodeBookmarksSnapshot(raw);
  if (snapshot === null) return null;
  const obj = asObject(raw);
  return {
    ...snapshot,
    applied: asNumber(obj?.applied, 0, Number.POSITIVE_INFINITY) ?? 0,
    skipped: asNumber(obj?.skipped, 0, Number.POSITIVE_INFINITY) ?? 0,
  };
}
