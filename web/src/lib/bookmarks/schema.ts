import type { VerseKey } from "$lib/data/quran";
import { asArray, asNumber, asObject, asString } from "$lib/storage";

/**
 * Bookmarks wire/domain contracts for the offline sync engine's "bookmarks"
 * domain. Wire JSON is decoded defensively at the boundary ($lib/storage
 * decoders); nothing from a response body is trusted past this module.
 */

export const SURAH_MIN = 1;
export const SURAH_MAX = 114;
/** Longest surah (Al-Baqarah). */
export const AYAH_MAX = 286;
export const FOLDER_NAME_MAX = 100;

export interface BookmarkFolder {
  readonly id: string;
  readonly name: string;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface Bookmark {
  readonly id: string;
  readonly folderId: string | null;
  readonly surah: number;
  readonly ayah: number;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export type BookmarksMutation =
  | {
      readonly kind: "bookmark.upsert";
      readonly id: string;
      readonly folderId: string | null;
      readonly surah: number;
      readonly ayah: number;
      readonly updatedAt: string;
    }
  | { readonly kind: "bookmark.delete"; readonly id: string; readonly updatedAt: string }
  | { readonly kind: "folder.upsert"; readonly id: string; readonly name: string; readonly updatedAt: string }
  | { readonly kind: "folder.delete"; readonly id: string; readonly updatedAt: string };

/** Full server state, as returned by POST /bookmark/v1/sync. */
export interface BookmarksSnapshot {
  readonly folders: readonly BookmarkFolder[];
  readonly bookmarks: readonly Bookmark[];
}

export interface BookmarksSyncEnvelope extends BookmarksSnapshot {
  readonly applied: number;
  readonly skipped: number;
}

/** Entity id (bookmark/folder row). Distinct from the engine's queue-entry mutation id. */
export function newBookmarkEntityId(): string {
  // `in` (not typeof): randomUUID is missing on insecure origins (LAN http dev).
  if ("randomUUID" in crypto) return crypto.randomUUID();
  return `bm-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

/**
 * Legacy reader VerseKey format: `${surah}:${ayah}` (see verseKey() in
 * $lib/data/quran and Record<VerseKey, boolean> in reader-core).
 */
export function verseKeyOf(surah: number, ayah: number): VerseKey {
  return `${surah}:${ayah}`;
}

/** Strict parse of the legacy VerseKey; null for anything not `${digits}:${digits}`. */
export function parseVerseKey(key: string): { surah: number; ayah: number } | null {
  const match = /^(\d+):(\d+)$/.exec(key);
  if (!match) return null;
  const surah = Number(match[1]);
  const ayah = Number(match[2]);
  return { surah, ayah };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed field from the sync response; this decoder is its boundary parser.
function decodeTimestamp(raw: unknown): string | undefined {
  const value = asString(raw);
  return value !== undefined && value.length > 0 ? value : undefined;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed folder row from the sync response; this decoder is its boundary parser.
export function decodeBookmarkFolder(raw: unknown): BookmarkFolder | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const name = asString(obj.name);
  if (id === undefined || id.length === 0) return null;
  if (name === undefined || name.length === 0 || name.length > FOLDER_NAME_MAX) return null;
  const createdAt = decodeTimestamp(obj.createdAt);
  const updatedAt = decodeTimestamp(obj.updatedAt);
  if (createdAt === undefined || updatedAt === undefined) return null;
  return { id, name, createdAt, updatedAt };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed bookmark row from the sync response; this decoder is its boundary parser.
export function decodeBookmark(raw: unknown): Bookmark | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  if (id === undefined || id.length === 0) return null;
  const surah = asNumber(obj.surah, SURAH_MIN, SURAH_MAX);
  const ayah = asNumber(obj.ayah, 1, AYAH_MAX);
  if (surah === undefined || ayah === undefined || !Number.isSafeInteger(surah) || !Number.isSafeInteger(ayah)) {
    return null;
  }
  const createdAt = decodeTimestamp(obj.createdAt);
  const updatedAt = decodeTimestamp(obj.updatedAt);
  if (createdAt === undefined || updatedAt === undefined) return null;
  let folderId: string | null | undefined;
  if (obj.folderId === null) folderId = null;
  else {
    const value = asString(obj.folderId);
    folderId = value !== undefined && value.length > 0 ? value : undefined;
  }
  if (folderId === undefined) return null;
  return { id, folderId, surah, ayah, createdAt, updatedAt };
}

/**
 * Decode the `{ applied, skipped, folders, bookmarks }` sync envelope. Counts
 * fall back to the mutation-batch size when the server omits or mangles them —
 * they are status bookkeeping, never data.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed JSON body of POST /bookmark/v1/sync; this decoder is its boundary parser.
export function decodeBookmarksEnvelope(raw: unknown): BookmarksSyncEnvelope | null {
  const obj = asObject(raw);
  if (!obj) return null;
  if (!Array.isArray(obj.folders) || !Array.isArray(obj.bookmarks)) return null;
  const applied = asNumber(obj.applied, 0, Number.POSITIVE_INFINITY);
  const skipped = asNumber(obj.skipped, 0, Number.POSITIVE_INFINITY);
  return {
    applied: applied ?? 0,
    skipped: skipped ?? 0,
    folders: asArray(obj.folders, (item) => decodeBookmarkFolder(item) ?? undefined),
    bookmarks: asArray(obj.bookmarks, (item) => decodeBookmark(item) ?? undefined),
  };
}

/** Decode a bare snapshot (no counts), e.g. for tests and future pull shapes. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is an unparsed snapshot payload; this decoder is its boundary parser.
export function decodeBookmarksSnapshot(raw: unknown): BookmarksSnapshot | null {
  const envelope = decodeBookmarksEnvelope(raw);
  if (envelope === null) return null;
  return { folders: envelope.folders, bookmarks: envelope.bookmarks };
}
