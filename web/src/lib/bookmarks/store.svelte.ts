import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import { deviceIdFor } from "#lib/device/identity.js";
import {
  asNumber,
  asObject,
  asString,
  readJSON,
  readRaw,
  removeJSON,
  tryWriteJSON,
  writeRaw,
} from "#lib/storage/index.js";
import { createClock, localClockStorage, type SyncClock } from "#lib/sync/clock.js";
import { newEntityId } from "#lib/sync/ids.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  SyncPausedError,
  type RegisteredSyncDomain,
  type SyncStatus,
} from "#lib/sync/index.js";
import { compareStamps, mergeBookmark, mergeFolder, type JsonObject } from "#lib/sync/merge.js";
import { mergedDirty } from "#lib/sync/round-status.js";

import { bookmarksDomainFor, createBookmarksDomain, type BookmarkAuthLike } from "./domain";
import {
  DEFAULT_FOLDER_NAME,
  FOLDER_NAME_MAX,
  compareFolders,
  decodeBookmarksStored,
  defaultFolderIdFor,
  isSingleVerseBookmark,
  liveKeyOf,
  parseVerseKey,
  type Bookmark,
  type BookmarkFolder,
  type BookmarkRowResult,
  type BookmarksRound,
  type BookmarksSnapshot,
  type BookmarksStored,
  type BookmarksWireRow,
  type FolderTombstone,
} from "./schema";

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SyncEngineLike {
  flush(domain?: string): Promise<void>;
  start(): () => void;
  setStorageDegraded?(on: boolean): void;
  readonly status: SyncStatus;
}

/** Auth status source; authState satisfies this structurally. */
export interface BookmarksAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
  /** Session-loss seam: probe confirms; anonymous ⇒ terminate transition + banner. */
  notifySessionLost?(): void;
}

export interface BookmarksStoreDeps {
  readonly engine: SyncEngineLike;
  readonly auth: BookmarkAuthLike;
  readonly state?: BookmarksAuthStateLike;
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

/** What a verse toggle did — the UI shows "Saved to …" or an Undo for the removal. */
export type ToggleResult =
  | { readonly kind: "added"; readonly folder: BookmarkFolder; readonly bookmark: Bookmark }
  | { readonly kind: "removed"; readonly bookmarks: readonly Bookmark[] }
  | { readonly kind: "noop" };

/** Everything a folder delete took with it, for Undo. */
export interface DeletedFolder {
  readonly folder: BookmarkFolder;
  readonly bookmarks: readonly Bookmark[];
}

/** What an import added. Folders and rows counted separately for the Settings toast. */
export interface BookmarkImportCounts {
  readonly folders: number;
  readonly bookmarks: number;
}

/**
 * Bookmarks. One store, two owners:
 *
 * - signed out → the device owner: folders + bookmarks live in
 *   `easyquran.bookmarks.local` and every edit writes through;
 * - signed in → the account: the row is the queue. Each edit stamps its register
 *   and persists `dirty` with the row; a round sends the dirty rows and merges
 *   what it pulls (contracts/sync.md), so nothing waits in memory. Only a tombstone
 *   deletes, and tombstones stay as clean rows until the cap trims them.
 *
 * On sign-in the device's bookmarks join the account silently (the device
 * default folder maps onto the account default); `onMerged` reports how many.
 */

const LOCAL_OWNER = "local";
const LOCAL_KEY = "easyquran.bookmarks.local";
const LOCAL_VERSION = 3;
const SYNC_PAGE = 200;
/** A device may hold this many clean tombstones. */
const TOMBSTONE_CAP = 500;
/** A cached account nobody signed into for this long loses its clean rows. */
const IDLE_ACCOUNT_MS = 180 * 24 * 60 * 60 * 1000;
/** First-pull backstop: a paused first round would otherwise wait for the 30s periodic sync. */
const FIRST_SYNC_RETRY_MS = 2_000;
const FIRST_SYNC_RETRY_MAX = 3;
const EPOCH_ISO = "1970-01-01T00:00:00.000Z";

function cacheKeyFor(ownerId: number): string {
  return `easyquran.bookmarks.user.${ownerId}`;
}

function lastFolderKeyFor(owner: string): string {
  return `easyquran.bookmarks.last-folder.${owner}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

function validFolderName(name: string): string | null {
  const trimmed = name.trim();
  return trimmed.length > 0 && trimmed.length <= FOLDER_NAME_MAX ? trimmed : null;
}

function seedDefault(id: string): BookmarkFolder {
  return {
    id,
    name: DEFAULT_FOLDER_NAME,
    isDefault: true,
    position: null,
    createdAt: EPOCH_ISO,
    updatedAt: EPOCH_ISO,
    nameStamp: null,
    positionStamp: null,
    deletedStamp: null,
    deleted: false,
    dirty: false,
    rejected: false,
  };
}

/** A stamp the server rewrote is below ours: adopt it. One it beat arrives with the row. */
function lowerOf(result: string | null | undefined, local: string | null): string | null {
  return result !== null && result !== undefined && local !== null && result < local
    ? result
    : local;
}

function bookmarkRegisters(row: Bookmark): JsonObject {
  return {
    id: row.id,
    register: {
      stamp: row.stamp,
      value: {
        context: { sourceId: row.sourceId },
        deleted: row.deleted,
        folderId: row.folderId,
        key: `${row.surah}:${row.ayah}-${row.endSurah ?? row.surah}:${row.endAyah ?? row.ayah}`,
      },
    },
  };
}

/** Whole-row register: the winning side's row, with flags merged. */
function mergeBookmarkRow(a: Bookmark, b: Bookmark): Bookmark {
  const merged = mergeBookmark(bookmarkRegisters(a), bookmarkRegisters(b));
  const winner = JSON.stringify(merged) === JSON.stringify(bookmarkRegisters(a)) ? a : b;
  const dirty = mergedDirty(
    { dirty: a.dirty, stamps: [a.stamp] },
    { dirty: b.dirty, stamps: [b.stamp] },
    [winner.stamp],
  );
  return { ...winner, dirty, rejected: a.rejected && b.rejected };
}

function folderRegisters(row: BookmarkFolder): JsonObject {
  return {
    id: row.id,
    isDefault: row.isDefault,
    name: { stamp: row.nameStamp, value: row.name },
    position: { stamp: row.positionStamp, value: row.position },
    deleted: { stamp: row.deletedStamp, value: row.deleted },
  };
}

function folderStamps(row: BookmarkFolder): readonly (string | null)[] {
  return [row.nameStamp, row.positionStamp, row.deletedStamp];
}

/** Name, position and deleted each merge alone. */
function mergeFolderRow(a: BookmarkFolder, b: BookmarkFolder): BookmarkFolder {
  const merged = mergeFolder(folderRegisters(a), folderRegisters(b));
  const register = (name: string) => {
    const pair = asObject(merged[name]);
    return { stamp: asString(pair?.stamp) ?? null, value: pair?.value };
  };
  const nameStamp = register("name").stamp;
  const positionStamp = register("position").stamp;
  const deletedStamp = register("deleted").stamp;
  return {
    ...a,
    name: asString(register("name").value) ?? a.name,
    position: asNumber(register("position").value, -1_000_000, 1_000_000) ?? null,
    deleted: register("deleted").value === true,
    isDefault: a.isDefault || b.isDefault,
    nameStamp,
    positionStamp,
    deletedStamp,
    updatedAt: a.updatedAt > b.updatedAt ? a.updatedAt : b.updatedAt,
    dirty: mergedDirty(
      { dirty: a.dirty, stamps: folderStamps(a) },
      { dirty: b.dirty, stamps: folderStamps(b) },
      [nameStamp, positionStamp, deletedStamp],
    ),
    rejected: a.rejected && b.rejected,
  };
}

function mergeRows<T extends { readonly id: string }>(
  a: readonly T[],
  b: readonly T[],
  merge: (x: T, y: T) => T,
): T[] {
  const byId = new Map(a.map((row) => [row.id, row]));
  for (const row of b) {
    const mine = byId.get(row.id);
    byId.set(row.id, mine === undefined ? row : merge(mine, row));
  }
  return [...byId.values()];
}

/** Clean tombstones beyond the cap go, oldest first. Live and dirty rows never do. */
function trimTombstones<T extends { readonly deleted: boolean; readonly dirty: boolean }>(
  rows: readonly T[],
): readonly T[] {
  const spare = rows.filter((row) => row.deleted && !row.dirty);
  if (spare.length <= TOMBSTONE_CAP) return rows;
  const dropped = new Set(spare.slice(0, spare.length - TOMBSTONE_CAP));
  return rows.filter((row) => !dropped.has(row));
}

export class BookmarksStore {
  readonly #engine: SyncEngineLike;
  readonly #auth: BookmarkAuthLike;
  readonly #state: BookmarksAuthStateLike;
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;

  /** Durable rows, tombstones included: the device envelope or the account's cache. */
  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  #owner = $state<string>(LOCAL_OWNER);
  #lastFolder = $state<string | null>(null);
  #syncedAt = $state<number | null>(null);
  #storageFailed = $state(false);
  #rejected = $state(0);
  #capped = $state(0);
  #updateRequired = $state(false);
  #suspended = $state(false);
  #hydrated = false;
  #generation = 0;
  #registered: string | null = null;
  #firstSyncRetryTimer: ReturnType<typeof setTimeout> | null = null;
  #firstSyncRetries = 0;
  #mergeListeners = new Set<(count: number) => void>();
  #since = 0;
  #after = -1;
  #epoch: string | null = null;
  #clock: SyncClock | null = null;
  #cappedIds = new Set<string>();
  /** Past the tombstone floor: ids seen in this full pull. Others are gone. */
  #fullPullSeen: Set<string> | null = null;

  constructor(deps: BookmarksStoreDeps) {
    this.#engine = deps.engine;
    this.#auth = deps.auth;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
    this.#unregister = deps.unregister ?? unregisterDomain;
  }

  // --- reads ---

  /** True when the view is an account's (synced) bookmarks. */
  get authed(): boolean {
    return !this.#suspended && this.#owner !== LOCAL_OWNER;
  }

  get defaultFolderId(): string {
    return this.#defaultIdFor(this.#owner);
  }

  /** Live folders, default first then manual order. Always holds the default folder. */
  get folders(): readonly BookmarkFolder[] {
    const live = this.#folders.filter((f) => !f.deleted);
    const defaultId = this.defaultFolderId;
    const withDefault = live.some((f) => f.id === defaultId)
      ? live
      : [...live, seedDefault(defaultId)];
    return [...withDefault].sort(compareFolders);
  }

  /**
   * Live bookmarks, newest first, with the S1 set rules applied to the view. An
   * older bookmark dies with a deleted folder and a newer one shows in the default
   * folder, and one key shows one row, the highest stamp.
   */
  get bookmarks(): readonly Bookmark[] {
    const deletedFolders = new Map(
      this.#folders.filter((f) => f.deleted).map((f) => [f.id, f.deletedStamp]),
    );
    const byKey = new Map<string, Bookmark>();
    for (const stored of this.#bookmarks) {
      if (stored.deleted) continue;
      const row = this.#placed(stored, deletedFolders);
      if (row === null) continue;
      const key = liveKeyOf(row);
      const prev = byKey.get(key);
      if (prev === undefined || this.#outranks(row, prev)) byKey.set(key, row);
    }
    return [...byKey.values()].sort((a, b) => (a.updatedAt > b.updatedAt ? -1 : 1));
  }

  #placed(row: Bookmark, deletedFolders: ReadonlyMap<string, string | null>): Bookmark | null {
    if (!deletedFolders.has(row.folderId)) return row;
    const stamp = deletedFolders.get(row.folderId) ?? null;
    if (compareStamps(row.stamp, stamp) <= 0) return null;
    return { ...row, folderId: this.defaultFolderId };
  }

  #outranks(a: Bookmark, b: Bookmark): boolean {
    const order = compareStamps(a.stamp, b.stamp);
    if (order !== 0) return order > 0;
    return a.updatedAt > b.updatedAt;
  }

  /** Where a new tap saves: the last folder used, else the default. */
  get targetFolder(): BookmarkFolder {
    const folders = this.folders;
    const last = this.#lastFolder;
    return (
      folders.find((f) => f.id === last) ??
      folders.find((f) => f.isDefault) ??
      folders[0] ??
      seedDefault(this.defaultFolderId)
    );
  }

  /** Rows waiting to reach the server. */
  get pending(): number {
    if (this.#accountId() === null) return 0;
    return [...this.#folders, ...this.#bookmarks].filter((row) => row.dirty && !row.rejected)
      .length;
  }

  get status(): SyncStatus {
    return { ...this.#engine.status, pending: this.pending };
  }

  /** Epoch ms of the last server snapshot for this account; null before the first. */
  get syncedAt(): number | null {
    return this.#syncedAt;
  }

  /** The browser refused a durable write. Never report those rows as synced. */
  get storageFailed(): boolean {
    return this.#storageFailed;
  }

  /** Rows the server refused, kept here and not resent until edited. */
  get rejectedCount(): number {
    return this.#rejected;
  }

  /** Rows waiting for room under the account's quota. */
  get cappedCount(): number {
    return this.#capped;
  }

  /** This build is below the server's minimum protocol. Reading keeps working. */
  get updateRequired(): boolean {
    return this.#updateRequired;
  }

  folderById(id: string): BookmarkFolder | undefined {
    return this.folders.find((folder) => folder.id === id);
  }

  bookmarksIn(folderId: string): readonly Bookmark[] {
    return this.bookmarks.filter((b) => b.folderId === folderId);
  }

  countIn(folderId: string): number {
    return this.bookmarksIn(folderId).length;
  }

  /** Single-verse rows for this verse, across every folder. */
  rowsFor(surah: number, ayah: number): readonly Bookmark[] {
    return this.bookmarks.filter(
      (b) => isSingleVerseBookmark(b) && b.surah === surah && b.ayah === ayah,
    );
  }

  isBookmarked(surah: number, ayah: number): boolean {
    return this.rowsFor(surah, ayah).length > 0;
  }

  isMarkedKey(key: string): boolean {
    const coords = parseVerseKey(key);
    return coords !== null && this.isBookmarked(coords.surah, coords.ayah);
  }

  /** Folder ids holding this verse. */
  foldersOf(surah: number, ayah: number): ReadonlySet<string> {
    return new Set(this.rowsFor(surah, ayah).map((b) => b.folderId));
  }

  isRangeMarked(surah: number, ayah: number, endSurah: number, endAyah: number): boolean {
    return this.bookmarks.some(
      (b) =>
        b.surah === surah &&
        b.ayah === ayah &&
        (b.endSurah ?? b.surah) === endSurah &&
        (b.endAyah ?? b.ayah) === endAyah,
    );
  }

  /** Sign-in merge notices ("Added N bookmarks from this device"). */
  onMerged(fn: (count: number) => void): () => void {
    this.#mergeListeners.add(fn);
    return () => this.#mergeListeners.delete(fn);
  }

  // --- writes ---

  /** Verse tap: unsaved → save to the target folder; saved anywhere → remove from every folder. */
  toggle(surah: number, ayah: number, sourceId: string | null = null): ToggleResult {
    if (this.#suspended) return { kind: "noop" };
    this.#syncOwner();
    const existing = this.rowsFor(surah, ayah);
    if (existing.length > 0) {
      for (const row of existing) this.#deleteBookmark(row.id);
      return { kind: "removed", bookmarks: existing };
    }
    const folder = this.targetFolder;
    const bookmark = this.#addBookmark(folder.id, surah, ayah, null, null, sourceId);
    return { kind: "added", folder, bookmark };
  }

  toggleVerse(key: string, sourceId: string | null = null): ToggleResult {
    const coords = parseVerseKey(key);
    if (coords === null) return { kind: "noop" };
    return this.toggle(coords.surah, coords.ayah, sourceId);
  }

  /** "Save to…" checkbox: put this verse in (or take it out of) one folder. */
  setInFolder(
    surah: number,
    ayah: number,
    folderId: string,
    on: boolean,
    sourceId: string | null = null,
  ): void {
    if (this.#suspended || this.folderById(folderId) === undefined) return;
    this.#syncOwner();
    const row = this.rowsFor(surah, ayah).find((b) => b.folderId === folderId);
    if (on && row === undefined) {
      this.#addBookmark(folderId, surah, ayah, null, null, sourceId);
      this.#rememberFolder(folderId);
    } else if (!on && row !== undefined) {
      this.#deleteBookmark(row.id);
    }
  }

  /** Range bookmark (snippets): toggles in the default folder. */
  toggleRange(surah: number, ayah: number, endSurah: number, endAyah: number): void {
    if (this.#suspended) return;
    this.#syncOwner();
    const existing = this.bookmarks.filter(
      (b) =>
        b.surah === surah &&
        b.ayah === ayah &&
        (b.endSurah ?? b.surah) === endSurah &&
        (b.endAyah ?? b.ayah) === endAyah,
    );
    if (existing.length > 0) {
      for (const row of existing) this.#deleteBookmark(row.id);
      return;
    }
    this.#addBookmark(this.defaultFolderId, surah, ayah, endSurah, endAyah, null);
  }

  remove(bookmarkId: string): Bookmark | null {
    const row = this.bookmarks.find((b) => b.id === bookmarkId) ?? null;
    if (row !== null) this.#deleteBookmark(row.id);
    return row;
  }

  /**
   * Bulk remove (selection bar). Each row is a normal write: stamped and dirty
   * when signed in. Returns the rows as they were, for `restore` on Undo.
   */
  removeMany(bookmarkIds: readonly string[]): Bookmark[] {
    if (this.#suspended) return [];
    const removed: Bookmark[] = [];
    for (const id of new Set(bookmarkIds)) {
      const row = this.remove(id);
      if (row !== null) removed.push(row);
    }
    return removed;
  }

  /**
   * Bulk move (selection bar). Rows already in the target folder are skipped.
   * Returns the rows as they were before the move; `restore` on Undo puts each
   * back in its old folder with a newer stamp, so the move reverts on every device.
   */
  moveMany(bookmarkIds: readonly string[], folderId: string): Bookmark[] {
    if (this.#suspended || this.folderById(folderId) === undefined) return [];
    const before: Bookmark[] = [];
    for (const id of new Set(bookmarkIds)) {
      const row = this.bookmarks.find((b) => b.id === id);
      if (row === undefined || row.folderId === folderId) continue;
      before.push(row);
      this.moveToFolder(row.id, folderId);
    }
    return before;
  }

  /** Undo for removals: same ids, fresh timestamps — newer than the tombstones. */
  restore(rows: readonly Bookmark[]): void {
    for (const row of rows) this.#putBookmark({ ...row, deleted: false, updatedAt: nowIso() });
  }

  moveToFolder(bookmarkId: string, folderId: string): void {
    const row = this.bookmarks.find((b) => b.id === bookmarkId);
    if (row === undefined || row.folderId === folderId || this.folderById(folderId) === undefined) {
      return;
    }
    const twin = this.bookmarks.find(
      (b) => b.folderId === folderId && liveKeyOf({ ...row, folderId }) === liveKeyOf(b),
    );
    // Already in the target folder: the move collapses into the existing row.
    if (twin !== undefined) this.#deleteBookmark(row.id);
    else this.#putBookmark({ ...row, folderId, updatedAt: nowIso() });
    this.#rememberFolder(folderId);
  }

  createFolder(name: string): BookmarkFolder | null {
    const trimmed = validFolderName(name);
    if (trimmed === null || this.#suspended) return null;
    this.#syncOwner();
    const now = nowIso();
    const folder: BookmarkFolder = {
      id: newEntityId(),
      name: trimmed,
      isDefault: false,
      position: null,
      createdAt: now,
      updatedAt: now,
      nameStamp: null,
      positionStamp: null,
      deletedStamp: null,
      deleted: false,
      dirty: false,
      rejected: false,
    };
    this.#putFolder(folder);
    return folder;
  }

  renameFolder(id: string, name: string): void {
    const trimmed = validFolderName(name);
    const folder = this.folderById(id);
    if (trimmed === null || folder === undefined || folder.name === trimmed) return;
    this.#putFolder({ ...folder, name: trimmed, updatedAt: nowIso() });
  }

  setFolderPosition(id: string, position: number | null): void {
    const folder = this.folderById(id);
    if (folder === undefined || folder.position === position) return;
    this.#putFolder({ ...folder, position, updatedAt: nowIso() });
  }

  /** Delete a folder and every bookmark in it (never the default). Returns what went, for Undo. */
  deleteFolder(id: string): DeletedFolder | null {
    const folder = this.folderById(id);
    if (folder === undefined || folder.isDefault || this.#suspended) return null;
    const bookmarks = this.bookmarksIn(id);
    if (this.#accountId() === null) {
      this.#folders = this.#folders.filter((f) => f.id !== id);
      this.#bookmarks = this.#bookmarks.filter((b) => b.folderId !== id);
      this.#writeLocal();
    } else {
      // Rows in it follow the folder's stamp, here and on the server.
      this.#putFolder({ ...folder, deleted: true, updatedAt: nowIso() });
    }
    if (this.#lastFolder === id) this.#rememberFolder(null);
    return { folder, bookmarks };
  }

  restoreFolder(deleted: DeletedFolder): void {
    this.#putFolder({ ...deleted.folder, deleted: false, updatedAt: nowIso() });
    this.restore(deleted.bookmarks);
  }

  /**
   * Import (F6). Only ids this store lacks are added, and each lands through the
   * normal write path, so it gets fresh stamps and `dirty`. Existing ids and
   * tombstones are skipped; a folder id this store lacks (or has deleted) files
   * into the default folder; a verse already live in that folder is skipped.
   */
  importRows(
    incomingFolders: readonly BookmarkFolder[],
    incomingBookmarks: readonly Bookmark[],
  ): BookmarkImportCounts {
    if (this.#suspended) return { folders: 0, bookmarks: 0 };
    this.#syncOwner();
    let folders = 0;
    for (const folder of incomingFolders) {
      const name = validFolderName(folder.name);
      if (folder.isDefault || name === null || this.#folders.some((f) => f.id === folder.id)) {
        continue;
      }
      this.#putFolder({
        ...folder,
        name,
        deleted: false,
        nameStamp: null,
        positionStamp: null,
        deletedStamp: null,
        updatedAt: nowIso(),
        dirty: false,
        rejected: false,
      });
      // A position set on import needs its own stamp; setFolderPosition stamps it.
      if (folder.position !== null) this.setFolderPosition(folder.id, folder.position);
      folders += 1;
    }
    let added = 0;
    const fallback = this.defaultFolderId;
    for (const row of incomingBookmarks) {
      if (this.#bookmarks.some((b) => b.id === row.id)) continue;
      const target = this.#folders.find((f) => f.id === row.folderId && !f.deleted);
      const candidate: Bookmark = {
        ...row,
        folderId: target?.id ?? fallback,
        stamp: null,
        deleted: false,
        dirty: false,
        rejected: false,
        updatedAt: nowIso(),
      };
      const twin = this.#bookmarks.some(
        (b) =>
          !b.deleted && b.folderId === candidate.folderId && liveKeyOf(b) === liveKeyOf(candidate),
      );
      if (twin) continue;
      this.#putBookmark(candidate);
      added += 1;
    }
    return { folders, bookmarks: added };
  }

  // --- lifecycle ---

  hydrate(): void {
    if (!browser || this.#hydrated) return;
    this.#hydrated = true;
    this.#load(this.#resolveOwner());
    this.#pruneIdleAccounts();
    this.onAuthChanged();
    window.addEventListener("storage", (event) => {
      if (event.key === LOCAL_KEY && this.#owner === LOCAL_OWNER) this.#load(LOCAL_OWNER);
      const ownerId = this.#accountId();
      if (ownerId !== null && event.key === cacheKeyFor(ownerId)) this.#mergeStored();
    });
  }

  /** Auth edge: swap owners; signing in folds the device's bookmarks into the account. */
  onAuthChanged(): void {
    if (!this.#hydrated) return;
    this.#suspended = false;
    this.#syncOwner();
    const ownerId = this.#accountId();
    if (ownerId !== null) void this.#beginSync(ownerId);
  }

  /** Cookie account changed under us: hide the view until auth settles. */
  suspendAuth(): void {
    this.#suspended = true;
    this.#unregisterCurrent();
  }

  /** Rows the next round sends: folders first, so a new bookmark finds its folder. */
  dirtyRows(): readonly BookmarksWireRow[] {
    const ready = <T extends { id: string; dirty: boolean; rejected: boolean }>(
      rows: readonly T[],
    ): T[] => {
      const dirty = rows.filter((row) => row.dirty && !row.rejected);
      return [
        ...dirty.filter((row) => !this.#cappedIds.has(row.id)),
        ...dirty.filter((row) => this.#cappedIds.has(row.id)),
      ];
    };
    const folders: BookmarksWireRow[] = ready(this.#folders).map((f) => ({
      kind: "folder.upsert",
      id: f.id,
      nameStamp: f.nameStamp,
      name: f.name,
      positionStamp: f.positionStamp,
      position: f.position,
      deletedStamp: f.deletedStamp,
      deleted: f.deleted,
      updatedAt: f.updatedAt,
    }));
    const bookmarks: BookmarksWireRow[] = ready(this.#bookmarks).map((b) => ({
      kind: "bookmark.upsert",
      id: b.id,
      stamp: b.stamp,
      folderId: b.folderId,
      surah: b.surah,
      ayah: b.ayah,
      endSurah: b.endSurah,
      endAyah: b.endAyah,
      sourceId: b.sourceId,
      deleted: b.deleted,
      updatedAt: b.updatedAt,
    }));
    return [...folders, ...bookmarks].slice(0, SYNC_PAGE);
  }

  /**
   * Apply one round durably. `dirty` clears only for a row whose current stamps are
   * the ones sent. A row missing from a response means nothing: only a tombstone
   * deletes.
   */
  applyRound(round: BookmarksRound, sent: readonly BookmarksWireRow[]) {
    this.#clock?.onServerTime(round.serverTime);
    this.#mergeStored();

    const epochChanged = this.#epoch !== null && round.syncEpoch !== this.#epoch;
    const aheadOfServer = this.#since > round.latestRev;
    const belowFloor = this.#since > 0 && this.#since < round.tombstoneFloor;
    const restored = epochChanged || aheadOfServer;

    const folders = new Map(this.#folders.map((f) => [f.id, f]));
    const marks = new Map(this.#bookmarks.map((b) => [b.id, b]));
    const sentById = new Map(sent.map((row) => [row.id, row]));
    for (const result of round.results) {
      this.#applyResult(result, sentById.get(result.id), folders, marks);
    }
    this.#rejected = [...folders.values(), ...marks.values()].filter((r) => r.rejected).length;
    this.#capped = this.#cappedIds.size;

    for (const row of round.folders) {
      this.#clock?.observe(row.nameStamp);
      this.#clock?.observe(row.positionStamp);
      this.#clock?.observe(row.deletedStamp);
      const mine = folders.get(row.id);
      folders.set(row.id, mine === undefined ? row : mergeFolderRow(mine, row));
      this.#fullPullSeen?.add(row.id);
    }
    for (const tombstone of round.deletedFolders) this.#applyFolderTombstone(tombstone, folders);
    for (const row of round.bookmarks) {
      this.#clock?.observe(row.stamp);
      const mine = marks.get(row.id);
      marks.set(row.id, mine === undefined ? row : mergeBookmarkRow(mine, row));
      this.#fullPullSeen?.add(row.id);
    }

    if (belowFloor) this.#fullPullSeen = new Set();
    if (restored) {
      // Rows and `rev` went backwards: this device pushes everything it holds
      // again, which heals writes the restore lost.
      for (const [id, row] of folders) folders.set(id, { ...row, dirty: true });
      for (const [id, row] of marks) marks.set(id, { ...row, dirty: true });
    }

    const reset = restored || belowFloor;
    const fullPull = this.#since === 0;
    if (reset) {
      this.#since = 0;
      this.#after = -1;
    } else if (round.more && fullPull) {
      this.#since = 0;
      this.#after = round.rev;
    } else {
      this.#since = round.rev;
      this.#after = -1;
    }
    if (!reset && !round.more && this.#fullPullSeen !== null) {
      // Past the floor, the server no longer holds tombstones for what is gone.
      const seen = this.#fullPullSeen;
      for (const [id, row] of folders) {
        if (!row.dirty && !seen.has(id)) folders.set(id, { ...row, deleted: true });
      }
      for (const [id, row] of marks) {
        if (!row.dirty && !seen.has(id)) marks.set(id, { ...row, deleted: true });
      }
      this.#fullPullSeen = null;
    }

    this.#folders = trimTombstones([...folders.values()]);
    this.#bookmarks = trimTombstones([...marks.values()]);
    this.#epoch = round.syncEpoch ?? this.#epoch;
    this.#syncedAt = Date.now();
    // `since` is saved with the rows it covers, so an interrupted pull resumes.
    this.#writeCache();

    const progressed = round.results.some(
      (r) => r.status === "applied" || r.status === "superseded",
    );
    const moreDirty = sent.length >= SYNC_PAGE && progressed && this.dirtyRows().length > 0;
    return { more: reset || round.more || moreDirty };
  }

  #applyResult(
    result: BookmarkRowResult,
    sent: BookmarksWireRow | undefined,
    folders: Map<string, BookmarkFolder>,
    marks: Map<string, Bookmark>,
  ): void {
    if (sent === undefined) return;
    this.#cappedIds.delete(result.id);
    if (result.status === "capped") {
      this.#cappedIds.add(result.id);
      return;
    }
    if (sent.kind === "bookmark.upsert") {
      const mine = marks.get(result.id);
      if (mine === undefined || mine.stamp !== sent.stamp) return;
      if (result.status === "rejected") {
        marks.set(result.id, { ...mine, rejected: true });
      } else if (result.stamps === null) {
        marks.set(result.id, { ...mine, deleted: true, dirty: false });
      } else {
        marks.set(result.id, {
          ...mine,
          stamp: lowerOf(result.stamps.row, mine.stamp),
          dirty: false,
          rejected: false,
        });
      }
      return;
    }
    const mine = folders.get(result.id);
    if (mine === undefined) return;
    const unchanged =
      mine.nameStamp === sent.nameStamp &&
      mine.positionStamp === sent.positionStamp &&
      mine.deletedStamp === sent.deletedStamp;
    if (!unchanged) return;
    if (result.status === "rejected") {
      folders.set(result.id, { ...mine, rejected: true });
    } else if (result.stamps === null) {
      folders.set(result.id, { ...mine, deleted: true, dirty: false });
    } else {
      folders.set(result.id, {
        ...mine,
        nameStamp: lowerOf(result.stamps.name, mine.nameStamp),
        positionStamp: lowerOf(result.stamps.position, mine.positionStamp),
        deletedStamp: lowerOf(result.stamps.deleted, mine.deletedStamp),
        dirty: false,
        rejected: false,
      });
    }
  }

  /** Delete only on a tombstone. A local undo with a newer stamp keeps its row. */
  #applyFolderTombstone(tombstone: FolderTombstone, folders: Map<string, BookmarkFolder>): void {
    this.#clock?.observe(tombstone.deletedStamp);
    const mine = folders.get(tombstone.id);
    if (mine === undefined) return;
    if (compareStamps(mine.deletedStamp, tombstone.deletedStamp) > 0) return;
    folders.set(tombstone.id, {
      ...mine,
      deleted: true,
      deletedStamp: tombstone.deletedStamp,
      nameStamp: tombstone.nameStamp ?? mine.nameStamp,
      positionStamp: tombstone.positionStamp ?? mine.positionStamp,
    });
  }

  // --- internals ---

  #addBookmark(
    folderId: string,
    surah: number,
    ayah: number,
    endSurah: number | null,
    endAyah: number | null,
    sourceId: string | null,
  ): Bookmark {
    const now = nowIso();
    const bookmark: Bookmark = {
      id: newEntityId(),
      folderId,
      surah,
      ayah,
      endSurah,
      endAyah,
      sourceId,
      createdAt: now,
      updatedAt: now,
      stamp: null,
      deleted: false,
      dirty: false,
      rejected: false,
    };
    this.#putBookmark(bookmark);
    return bookmark;
  }

  #putBookmark(row: Bookmark): void {
    if (this.#accountId() === null) {
      this.#bookmarks = [
        ...this.#bookmarks.filter((b) => b.id !== row.id && liveKeyOf(b) !== liveKeyOf(row)),
        row,
      ];
      this.#writeLocal();
      return;
    }
    const stamped: Bookmark = {
      ...row,
      stamp: this.#clock?.stamp() ?? row.stamp,
      dirty: true,
      rejected: false,
    };
    this.#bookmarks = [...this.#bookmarks.filter((b) => b.id !== row.id), stamped];
    this.#afterAccountWrite();
  }

  #deleteBookmark(id: string): void {
    if (this.#accountId() === null) {
      this.#bookmarks = this.#bookmarks.filter((b) => b.id !== id);
      this.#writeLocal();
      return;
    }
    const row = this.#bookmarks.find((b) => b.id === id);
    if (row === undefined) return;
    this.#putBookmark({ ...row, deleted: true, updatedAt: nowIso() });
  }

  /** Only a register whose value changed gets a new stamp. */
  #putFolder(folder: BookmarkFolder): void {
    if (this.#accountId() === null) {
      this.#folders = [...this.#folders.filter((f) => f.id !== folder.id), folder];
      this.#writeLocal();
      return;
    }
    const prev = this.#folders.find((f) => f.id === folder.id);
    const next = (changed: boolean, stamp: string | null): string | null =>
      changed ? (this.#clock?.stamp() ?? stamp) : stamp;
    const stamped: BookmarkFolder = {
      ...folder,
      nameStamp: next(prev === undefined || prev.name !== folder.name, folder.nameStamp),
      positionStamp: next(
        prev !== undefined && prev.position !== folder.position,
        folder.positionStamp,
      ),
      deletedStamp: next(
        prev !== undefined && prev.deleted !== folder.deleted,
        folder.deletedStamp,
      ),
      dirty: true,
      rejected: false,
    };
    this.#folders = [...this.#folders.filter((f) => f.id !== folder.id), stamped];
    this.#afterAccountWrite();
  }

  /** The dirty flag lands in the same write as the row, then a round picks it up. */
  #afterAccountWrite(): void {
    this.#writeCache();
    const ownerId = this.#accountId();
    if (ownerId === null || this.#registered !== bookmarksDomainFor(ownerId)) return;
    void this.#engine.flush(bookmarksDomainFor(ownerId)).catch(() => undefined);
  }

  #rememberFolder(id: string | null): void {
    this.#lastFolder = id;
    writeRaw("local", lastFolderKeyFor(this.#owner), id ?? "");
  }

  #defaultIdFor(owner: string): string {
    return owner === LOCAL_OWNER ? defaultFolderIdFor("local") : defaultFolderIdFor(Number(owner));
  }

  #resolveOwner(): string {
    if (this.#state.authenticated && this.#state.user !== null) return String(this.#state.user.id);
    return LOCAL_OWNER;
  }

  #accountId(): number | null {
    if (this.#suspended || this.#owner === LOCAL_OWNER) return null;
    const id = Number(this.#owner);
    return Number.isSafeInteger(id) && id > 0 ? id : null;
  }

  #syncOwner(): void {
    if (!this.#hydrated) return;
    const next = this.#resolveOwner();
    if (next === this.#owner) return;
    const absorbing = this.#owner === LOCAL_OWNER && next !== LOCAL_OWNER;
    const local = absorbing ? { folders: this.#folders, bookmarks: this.#bookmarks } : null;
    this.#load(next);
    if (local !== null) this.#absorb(local);
  }

  /** Swap the in-memory view to `owner` (device envelope or cached account snapshot). */
  #load(owner: string): void {
    this.#unregisterCurrent();
    this.#generation += 1;
    this.#owner = owner;
    this.#syncedAt = null;
    this.#cappedIds.clear();
    this.#fullPullSeen = null;
    if (this.#firstSyncRetryTimer !== null) clearTimeout(this.#firstSyncRetryTimer);
    this.#firstSyncRetryTimer = null;
    this.#firstSyncRetries = 0;
    const last = readRaw("local", lastFolderKeyFor(owner));
    this.#lastFolder = last !== null && last.length > 0 ? last : null;
    const accountId = owner === LOCAL_OWNER ? null : owner;
    this.#clock =
      accountId === null ? null : createClock(deviceIdFor(accountId), localClockStorage(accountId));
    const raw = owner === LOCAL_OWNER ? readJSON(LOCAL_KEY) : readJSON(cacheKeyFor(Number(owner)));
    const stored = decodeBookmarksStored(raw);
    this.#folders = stored?.folders ?? [];
    this.#bookmarks = stored?.bookmarks ?? [];
    this.#since = stored?.since ?? 0;
    this.#after = stored?.after ?? -1;
    this.#epoch = stored?.epoch ?? null;
    this.#rejected = [...this.#folders, ...this.#bookmarks].filter((r) => r.rejected).length;
    this.#capped = 0;
    if (owner === LOCAL_OWNER && !this.#folders.some((f) => f.isDefault)) {
      this.#folders = [seedDefault(this.#defaultIdFor(owner)), ...this.#folders];
    }
    if (accountId !== null) this.#writeCache();
  }

  /** Another tab of this account wrote: fold its rows in. */
  #mergeStored(): void {
    const ownerId = this.#accountId();
    if (ownerId === null) return;
    const stored = decodeBookmarksStored(readJSON(cacheKeyFor(ownerId)));
    if (stored === null) return;
    this.#folders = mergeRows(this.#folders, stored.folders, mergeFolderRow);
    this.#bookmarks = mergeRows(this.#bookmarks, stored.bookmarks, mergeBookmarkRow);
    this.#since = this.#since === 0 ? 0 : Math.max(this.#since, stored.since);
    this.#epoch = this.#epoch ?? stored.epoch;
  }

  /** Other accounts' clean rows go after 180 days away. Dirty rows wait for sign-in. */
  #pruneIdleAccounts(): void {
    const prefix = "easyquran.bookmarks.user.";
    const now = Date.now();
    try {
      for (const key of Object.keys(localStorage).filter((k) => k.startsWith(prefix))) {
        const stored = decodeBookmarksStored(readJSON(key));
        if (stored === null || stored.lastSeenAt === 0) continue;
        if (key === cacheKeyFor(this.#accountId() ?? -1)) continue;
        if (now - stored.lastSeenAt < IDLE_ACCOUNT_MS) continue;
        tryWriteJSON(key, {
          ...stored,
          folders: stored.folders.filter((f) => f.dirty),
          bookmarks: stored.bookmarks.filter((b) => b.dirty),
        });
      }
    } catch {
      return;
    }
  }

  #writeLocal(): void {
    this.#noteWrite(
      tryWriteJSON(LOCAL_KEY, {
        v: LOCAL_VERSION,
        folders: this.#folders,
        bookmarks: this.#bookmarks,
      }),
    );
  }

  #writeCache(): void {
    const ownerId = this.#accountId();
    if (ownerId === null) return;
    const stored: BookmarksStored & { readonly v: number } = {
      v: LOCAL_VERSION,
      folders: this.#folders,
      bookmarks: this.#bookmarks,
      since: this.#since,
      after: this.#after,
      epoch: this.#epoch,
      lastSeenAt: Date.now(),
    };
    this.#noteWrite(tryWriteJSON(cacheKeyFor(ownerId), stored));
  }

  #noteWrite(landed: boolean): void {
    if (landed === this.#storageFailed) {
      this.#storageFailed = !landed;
      this.#engine.setStorageDegraded?.(!landed);
    }
  }

  /** Sign-in: device rows join the account; the device default maps onto the account default. */
  #absorb(local: BookmarksSnapshot): void {
    const localDefault = defaultFolderIdFor("local");
    const accountDefault = this.defaultFolderId;
    const remap = (id: string): string => (id === localDefault ? accountDefault : id);
    // Each row folds in with a stamp from its own time, so a newer edit elsewhere wins.
    const at = (iso: string): string | null => {
      const ms = Date.parse(iso);
      return this.#clock?.stampAt(Number.isFinite(ms) ? ms : 0) ?? null;
    };
    const folders: BookmarkFolder[] = [];
    for (const folder of local.folders) {
      if (folder.isDefault) continue;
      const stamp = at(folder.updatedAt);
      folders.push({ ...folder, nameStamp: stamp, positionStamp: stamp, dirty: true });
    }
    // A renamed device default carries its name to the account default.
    const renamed = local.folders.find((f) => f.isDefault && f.name !== DEFAULT_FOLDER_NAME);
    if (renamed !== undefined) {
      folders.push({
        ...seedDefault(accountDefault),
        name: renamed.name,
        updatedAt: renamed.updatedAt,
        nameStamp: at(renamed.updatedAt),
        dirty: true,
      });
    }
    const marks = local.bookmarks.map((row) => ({
      ...row,
      folderId: remap(row.folderId),
      stamp: at(row.updatedAt),
      dirty: true,
    }));
    this.#folders = mergeRows(this.#folders, folders, mergeFolderRow);
    this.#bookmarks = mergeRows(this.#bookmarks, marks, mergeBookmarkRow);
    this.#writeCache();
    removeJSON(LOCAL_KEY);
    const count = local.bookmarks.length;
    if (count > 0) for (const fn of this.#mergeListeners) fn(count);
  }

  #unregisterCurrent(): void {
    if (this.#registered === null) return;
    this.#unregister(this.#registered);
    this.#registered = null;
  }

  async #beginSync(ownerId: number): Promise<void> {
    const name = bookmarksDomainFor(ownerId);
    if (this.#registered === name) return;
    const generation = this.#generation;
    const isCurrent = (): boolean =>
      generation === this.#generation && this.#accountId() === ownerId && this.#registered === name;
    const domain = createBookmarksDomain(
      this.#auth,
      this.#state,
      ownerId,
      {
        dirtyRows: () => this.dirtyRows(),
        position: () => ({ since: this.#since, after: this.#after }),
        applyRound: (round, sent) => this.applyRound(round, sent),
        onUpgradeRequired: () => {
          this.#updateRequired = true;
        },
      },
      isCurrent,
      () => {
        if (!isCurrent()) return;
        this.suspendAuth();
        this.#state.notifySessionLost?.();
      },
    );
    const roundOf = domain.round?.bind(domain);
    const adapter: RegisteredSyncDomain = {
      ...domain,
      round: async (signal) => {
        if (roundOf === undefined) return { more: false };
        try {
          return await roundOf(signal);
        } catch (err) {
          if (err instanceof SyncPausedError) this.#scheduleFirstSyncRetry(ownerId, generation);
          throw err;
        }
      },
    };
    this.#register(adapter);
    this.#registered = name;
    this.#engine.start();
    await this.#engine.flush(name).catch(() => undefined);
  }

  #scheduleFirstSyncRetry(ownerId: number, generation: number): void {
    if (this.#firstSyncRetryTimer !== null || this.#syncedAt !== null) return;
    if (this.#firstSyncRetries >= FIRST_SYNC_RETRY_MAX) return;
    this.#firstSyncRetries += 1;
    this.#firstSyncRetryTimer = setTimeout(() => {
      this.#firstSyncRetryTimer = null;
      if (generation !== this.#generation || this.#accountId() !== ownerId) return;
      void this.#engine.flush(bookmarksDomainFor(ownerId)).catch(() => undefined);
    }, FIRST_SYNC_RETRY_MS);
  }
}

export function createBookmarksStore(deps: BookmarksStoreDeps): BookmarksStore {
  return new BookmarksStore(deps);
}

export const bookmarks: BookmarksStore = createBookmarksStore({
  engine: syncEngine,
  auth: authClient,
});
