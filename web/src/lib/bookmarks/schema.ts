import type { VerseKey } from "#lib/data/quran.js";
import { asArray, asLiteral, asNumber, asObject, asString } from "#lib/storage/index.js";
import { uuidV5 } from "#lib/sync/ids.js";

/**
 * Folder-based bookmarks. Every bookmark lives in a
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
  /** One stamp per register; null until the row first belongs to an account. */
  readonly nameStamp: string | null;
  readonly positionStamp: string | null;
  readonly deletedStamp: string | null;
  readonly deleted: boolean;
  /** The row is the queue: true until a result acknowledges these exact stamps. */
  readonly dirty: boolean;
  readonly rejected: boolean;
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
  /** Whole-row register stamp; null until the row first belongs to an account. */
  readonly stamp: string | null;
  readonly deleted: boolean;
  readonly dirty: boolean;
  readonly rejected: boolean;
}

export type BookmarksWireRow =
  | {
      readonly kind: "bookmark.upsert";
      readonly id: string;
      readonly stamp: string | null;
      readonly folderId: string;
      readonly surah: number;
      readonly ayah: number;
      readonly endSurah: number | null;
      readonly endAyah: number | null;
      readonly sourceId: string | null;
      readonly deleted: boolean;
      readonly updatedAt: string;
    }
  | {
      readonly kind: "folder.upsert";
      readonly id: string;
      readonly nameStamp: string | null;
      readonly name: string;
      readonly positionStamp: string | null;
      readonly position: number | null;
      readonly deletedStamp: string | null;
      readonly deleted: boolean;
      readonly updatedAt: string;
    };

export type BookmarkRowStatus = "applied" | "superseded" | "rejected" | "capped";

export interface BookmarkRowResult {
  readonly id: string;
  readonly status: BookmarkRowStatus;
  /** Null when the row is older than the tombstone floor. */
  readonly stamps: Partial<Record<"row" | "name" | "position" | "deleted", string | null>> | null;
}

export interface BookmarksSnapshot {
  readonly folders: readonly BookmarkFolder[];
  readonly bookmarks: readonly Bookmark[];
}

export interface BookmarksStored extends BookmarksSnapshot {
  readonly since: number;
  readonly after: number;
  readonly epoch: string | null;
  readonly lastSeenAt: number;
}

export interface BookmarksRound extends BookmarksSnapshot {
  readonly serverTime: number;
  readonly syncEpoch: string | null;
  readonly results: readonly BookmarkRowResult[];
  /** Tombstone rows: stamps and identity only. */
  readonly deletedFolders: readonly FolderTombstone[];
  readonly rev: number;
  readonly latestRev: number;
  readonly more: boolean;
  readonly tombstoneFloor: number;
}

export interface FolderTombstone {
  readonly id: string;
  readonly nameStamp: string | null;
  readonly positionStamp: string | null;
  readonly deletedStamp: string | null;
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

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed nullable stamp; this helper is its boundary parser.
function stampOrNull(raw: unknown): string | null {
  const value = asString(raw);
  return value !== undefined && value.length > 0 ? value : null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed folder row (wire or storage); this decoder is its boundary parser.
export function decodeBookmarkFolder(raw: unknown): BookmarkFolder | null {
  const obj = asObject(raw);
  if (!obj) return null;
  const id = asString(obj.id);
  const name = asString(obj.name);
  if (id === undefined || id.length !== 36) return null;
  if (name === undefined || name.length === 0 || name.length > FOLDER_NAME_MAX) return null;
  const updatedAt = decodeTimestamp(obj.updatedAt);
  if (updatedAt === undefined) return null;
  return {
    id,
    name,
    isDefault: obj.isDefault === true,
    position: asNumber(obj.position, -1_000_000, 1_000_000) ?? null,
    createdAt: decodeTimestamp(obj.createdAt) ?? updatedAt,
    updatedAt,
    nameStamp: stampOrNull(obj.nameStamp),
    positionStamp: stampOrNull(obj.positionStamp),
    deletedStamp: stampOrNull(obj.deletedStamp),
    deleted: obj.deleted === true,
    dirty: obj.dirty === true,
    rejected: obj.rejected === true,
  };
}

/** A range end equal to its start is a single verse: one shape for one row. */
function normalizedEnd(
  surah: number,
  ayah: number,
  endSurah: number | undefined,
  endAyah: number | undefined,
) {
  if (endSurah === undefined || endAyah === undefined) return { endSurah: null, endAyah: null };
  return endSurah === surah && endAyah === ayah ? { endSurah: null, endAyah: null } : { endSurah, endAyah };
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
  const deleted = obj.deleted === true;
  // A tombstone carries its key and stamps only.
  const updatedAt = decodeTimestamp(obj.updatedAt) ?? (deleted ? "1970-01-01T00:00:00.000Z" : undefined);
  if (updatedAt === undefined) return null;
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
    createdAt: decodeTimestamp(obj.createdAt) ?? updatedAt,
    updatedAt,
    sourceId: sourceId !== undefined && sourceId.length > 0 ? sourceId : null,
    ...normalizedEnd(surah, ayah, endSurah, endAyah),
    stamp: stampOrNull(obj.stamp),
    deleted,
    dirty: obj.dirty === true,
    rejected: obj.rejected === true,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed folder tombstone from a pull; this decoder is its boundary parser.
function decodeFolderTombstone(raw: unknown): FolderTombstone | undefined {
  const obj = asObject(raw);
  const id = asString(obj?.id);
  if (obj === undefined || id === undefined || obj.deleted !== true) return undefined;
  return {
    id,
    nameStamp: stampOrNull(obj.nameStamp),
    positionStamp: stampOrNull(obj.positionStamp),
    deletedStamp: stampOrNull(obj.deletedStamp),
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed per-owner localStorage value; this decoder is its boundary parser.
export function decodeBookmarksStored(raw: unknown): BookmarksStored | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.folders) || !Array.isArray(obj.bookmarks)) return null;
  const bound = Number.MAX_SAFE_INTEGER;
  return {
    folders: asArray(obj.folders, (row) => decodeBookmarkFolder(row) ?? undefined),
    bookmarks: asArray(obj.bookmarks, (row) => decodeBookmark(row) ?? undefined),
    since: asNumber(obj.since, 0, bound) ?? 0,
    after: asNumber(obj.after, -1, bound) ?? -1,
    epoch: stampOrNull(obj.epoch),
    lastSeenAt: asNumber(obj.lastSeenAt, 0, bound) ?? 0,
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed per-row result; this decoder is its boundary parser.
function decodeResult(raw: unknown): BookmarkRowResult | undefined {
  const obj = asObject(raw);
  const id = asString(obj?.id);
  const status = asLiteral<BookmarkRowStatus>(obj?.status, [
    "applied",
    "superseded",
    "rejected",
    "capped",
  ]);
  if (obj === undefined || id === undefined || status === undefined) return undefined;
  const stamps = asObject(obj.stamps);
  return {
    id,
    status,
    stamps:
      stamps === undefined
        ? null
        : {
            row: stampOrNull(stamps.row),
            name: stampOrNull(stamps.name),
            position: stampOrNull(stamps.position),
            deleted: stampOrNull(stamps.deleted),
          },
  };
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed POST /bookmark/v1/sync body; this decoder is its boundary parser.
export function decodeBookmarksRound(raw: unknown): BookmarksRound | null {
  const obj = asObject(raw);
  if (!obj || !Array.isArray(obj.folders) || !Array.isArray(obj.bookmarks)) return null;
  const bound = Number.MAX_SAFE_INTEGER;
  return {
    serverTime: asNumber(obj.serverTime, 0, bound) ?? Date.now(),
    syncEpoch: stampOrNull(obj.syncEpoch),
    results: asArray(obj.results, decodeResult),
    folders: asArray(obj.folders, (row) => decodeBookmarkFolder(row) ?? undefined),
    bookmarks: asArray(obj.bookmarks, (row) => decodeBookmark(row) ?? undefined),
    deletedFolders: asArray(obj.folders, decodeFolderTombstone),
    rev: asNumber(obj.rev, 0, bound) ?? 0,
    latestRev: asNumber(obj.latestRev, 0, bound) ?? 0,
    more: obj.more === true,
    tombstoneFloor: asNumber(obj.tombstoneFloor, 0, bound) ?? 0,
  };
}
