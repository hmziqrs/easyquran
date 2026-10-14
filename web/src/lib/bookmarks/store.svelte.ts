import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import {
  readJSON,
  readRaw,
  removeJSON,
  trailingDebounce,
  writeJSON,
  writeRaw,
} from "#lib/storage/index.js";
import { newEntityId } from "#lib/sync/ids.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  SyncPausedError,
  type RegisteredSyncDomain,
  type SyncDomain,
  type SyncMutation,
  type SyncStatus,
} from "#lib/sync/index.js";
import {
  SYNC_DRAIN_CHANNEL,
  SYNC_PAGE_NONCE,
  type SyncDrainMessage,
} from "#lib/sync/engine.svelte.js";

import { bookmarksDomainFor, createBookmarksDomain, type BookmarkAuthLike } from "./domain";
import {
  DEFAULT_FOLDER_NAME,
  FOLDER_NAME_MAX,
  compareFolders,
  decodeBookmarksSnapshot,
  defaultFolderIdFor,
  isSingleVerseBookmark,
  liveKeyOf,
  parseVerseKey,
  type Bookmark,
  type BookmarkFolder,
  type BookmarksMutation,
  type BookmarksSnapshot,
} from "./schema";

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SyncEngineLike {
  enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>>;
  pendingFor(domain: string): number;
  flush(domain?: string): Promise<void>;
  start(): () => void;
  hydrate?(): Promise<void>;
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

/**
 * Bookmarks (docs/reading-model.md §3). One store, two owners:
 *
 * - signed out → the device owner: folders + bookmarks live in
 *   `easyquran.bookmarks.local` and every edit writes through;
 * - signed in → the account: edits apply optimistically, queue in the
 *   `bookmarks.user:{id}` outbox partition, and the server snapshot is truth
 *   (pending overlays win until their mutation drains). The last snapshot is
 *   cached per account so the list paints instantly and works offline.
 *
 * On sign-in the device's bookmarks join the account silently (the device
 * default folder maps onto the account default); `onMerged` reports how many.
 */

const LOCAL_OWNER = "local";
const LOCAL_KEY = "easyquran.bookmarks.local";
const LOCAL_VERSION = 2;
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

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter narrows it to a displayable message string.
function errorMessage(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
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
  };
}

/** Narrow an untrusted cross-tab payload: a drain notice is `{domain, ids[]}`. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- data is the raw BroadcastChannel message payload; this decoder is its boundary parser.
function decodeDrainMessage(data: unknown): SyncDrainMessage | null {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-object is the only discriminator for the raw BroadcastChannel payload at this parse boundary.
  if (typeof data !== "object" || data === null) return null;
  if (!("domain" in data) || !("ids" in data)) return null;
  // SAFETY: property existence is checked above; this narrows the fields for validation only.
  const { domain, ids, page } = data as { domain: unknown; ids: unknown; page?: unknown };
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- domain is a wire string field with no static type before this check.
  if (typeof domain !== "string" || !Array.isArray(ids)) return null;
  if (page === SYNC_PAGE_NONCE) return null;
  for (const id of ids) {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- each ids element is an opaque wire value until this string check.
    if (typeof id !== "string") return null;
  }
  return { domain, ids };
}

export class BookmarksStore {
  readonly #engine: SyncEngineLike;
  readonly #auth: BookmarkAuthLike;
  readonly #state: BookmarksAuthStateLike;
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;

  /** Durable rows: the device envelope, or the account's last server snapshot. */
  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  /** Account overlays: entity id → row (upsert) or null (delete), until drained. */
  #pendingBookmarks = $state.raw<ReadonlyMap<string, Bookmark | null>>(new Map());
  #pendingFolders = $state.raw<ReadonlyMap<string, BookmarkFolder | null>>(new Map());
  #owner = $state<string>(LOCAL_OWNER);
  #lastFolder = $state<string | null>(null);
  #syncedAt = $state<number | null>(null);
  #enqueueError = $state<string | null>(null);
  #suspended = $state(false);
  #hydrated = false;
  #generation = 0;
  #registered: string | null = null;
  #mutationEntityIds = new Map<string, string>();
  #drainChannel: BroadcastChannel | null = null;
  #firstSyncRetryTimer: ReturnType<typeof setTimeout> | null = null;
  #firstSyncRetries = 0;
  #mergeListeners = new Set<(count: number) => void>();
  #persistCache = trailingDebounce(() => this.#writeCache(), 400);

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
    const byId = new Map(this.#folders.map((f) => [f.id, f]));
    for (const [id, row] of this.#pendingFolders) {
      if (row === null) byId.delete(id);
      else byId.set(id, row);
    }
    const defaultId = this.defaultFolderId;
    if (!byId.has(defaultId)) byId.set(defaultId, seedDefault(defaultId));
    return [...byId.values()].sort(compareFolders);
  }

  /** Live bookmarks whose folder is live, newest first. */
  get bookmarks(): readonly Bookmark[] {
    const folderIds = new Set(this.folders.map((f) => f.id));
    const byId = new Map(this.#bookmarks.map((b) => [b.id, b]));
    for (const [id, row] of this.#pendingBookmarks) {
      if (row === null) byId.delete(id);
      else byId.set(id, row);
    }
    // One live row per (folder, range): a pending row beats a stale snapshot twin.
    const byKey = new Map<string, Bookmark>();
    for (const row of byId.values()) {
      if (!folderIds.has(row.folderId)) continue;
      const key = liveKeyOf(row);
      const prev = byKey.get(key);
      if (
        prev === undefined ||
        this.#pendingBookmarks.has(row.id) ||
        row.updatedAt > prev.updatedAt
      ) {
        byKey.set(key, row);
      }
    }
    return [...byKey.values()].sort((a, b) => (a.updatedAt > b.updatedAt ? -1 : 1));
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

  get pending(): number {
    const ownerId = this.#accountId();
    if (ownerId === null || this.#suspended) return 0;
    return this.#engine.pendingFor(bookmarksDomainFor(ownerId));
  }

  get status(): SyncStatus {
    return { ...this.#engine.status, pending: this.pending };
  }

  /** Epoch ms of the last server snapshot for this account; null before the first. */
  get syncedAt(): number | null {
    return this.#syncedAt;
  }

  get enqueueError(): string | null {
    return this.#enqueueError;
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

  /** Undo for removals: same ids, fresh timestamps — newer than the tombstones. */
  restore(rows: readonly Bookmark[]): void {
    for (const row of rows) this.#putBookmark({ ...row, updatedAt: nowIso() });
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
    const now = nowIso();
    if (this.#accountId() === null) {
      this.#folders = this.#folders.filter((f) => f.id !== id);
      this.#bookmarks = this.#bookmarks.filter((b) => b.folderId !== id);
      this.#writeLocal();
    } else {
      // The server cascades the tombstone to the folder's bookmarks.
      this.#overlayFolder(id, null);
      for (const row of bookmarks) this.#overlayBookmark(row.id, null);
      this.#enqueue({ kind: "folder.delete", id, updatedAt: now });
    }
    if (this.#lastFolder === id) this.#rememberFolder(null);
    return { folder, bookmarks };
  }

  restoreFolder(deleted: DeletedFolder): void {
    this.#putFolder({ ...deleted.folder, updatedAt: nowIso() });
    this.restore(deleted.bookmarks);
  }

  // --- lifecycle ---

  hydrate(): void {
    if (!browser || this.#hydrated) return;
    this.#hydrated = true;
    this.#installDrainListener();
    void this.#engine.hydrate?.();
    this.#load(this.#resolveOwner());
    this.onAuthChanged();
    window.addEventListener("storage", (event) => {
      if (event.key === LOCAL_KEY && this.#owner === LOCAL_OWNER) this.#load(LOCAL_OWNER);
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

  applyServer(
    snapshot: BookmarksSnapshot,
    drained?: readonly SyncMutation<BookmarksMutation>[],
  ): void {
    if (drained !== undefined)
      this.#retire(
        drained.map((m) => m.payload.id),
        drained.map((m) => m.id),
      );
    this.#folders = snapshot.folders;
    this.#bookmarks = snapshot.bookmarks;
    this.#syncedAt = Date.now();
    this.#persistCache.schedule();
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
    this.#overlayBookmark(row.id, row);
    this.#enqueue({
      kind: "bookmark.upsert",
      id: row.id,
      folderId: row.folderId,
      surah: row.surah,
      ayah: row.ayah,
      endSurah: row.endSurah,
      endAyah: row.endAyah,
      sourceId: row.sourceId,
      updatedAt: row.updatedAt,
    });
  }

  #deleteBookmark(id: string): void {
    if (this.#accountId() === null) {
      this.#bookmarks = this.#bookmarks.filter((b) => b.id !== id);
      this.#writeLocal();
      return;
    }
    this.#overlayBookmark(id, null);
    this.#enqueue({ kind: "bookmark.delete", id, updatedAt: nowIso() });
  }

  #putFolder(folder: BookmarkFolder): void {
    if (this.#accountId() === null) {
      this.#folders = [...this.#folders.filter((f) => f.id !== folder.id), folder];
      this.#writeLocal();
      return;
    }
    this.#overlayFolder(folder.id, folder);
    this.#enqueue({
      kind: "folder.upsert",
      id: folder.id,
      name: folder.name,
      position: folder.position,
      updatedAt: folder.updatedAt,
    });
  }

  #overlayBookmark(id: string, row: Bookmark | null): void {
    const next = new Map(this.#pendingBookmarks);
    next.set(id, row);
    this.#pendingBookmarks = next;
  }

  #overlayFolder(id: string, row: BookmarkFolder | null): void {
    const next = new Map(this.#pendingFolders);
    next.set(id, row);
    this.#pendingFolders = next;
  }

  /** Drop overlays whose mutations drained (here or in another tab). */
  #retire(entityIds: readonly string[], mutationIds: readonly string[]): void {
    if (entityIds.length === 0) return;
    const bookmarks = new Map(this.#pendingBookmarks);
    const folders = new Map(this.#pendingFolders);
    for (const id of entityIds) {
      bookmarks.delete(id);
      folders.delete(id);
    }
    for (const id of mutationIds) this.#mutationEntityIds.delete(id);
    this.#pendingBookmarks = bookmarks;
    this.#pendingFolders = folders;
  }

  #enqueue(payload: BookmarksMutation): void {
    const ownerId = this.#accountId();
    if (ownerId === null) return;
    const generation = this.#generation;
    void this.#engine.enqueue(bookmarksDomainFor(ownerId), payload).then(
      (mutation) => {
        if (generation !== this.#generation) return;
        this.#mutationEntityIds.set(mutation.id, payload.id);
        this.#enqueueError = null;
      },
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); errorMessage() narrows it.
      (err: unknown) => {
        if (generation !== this.#generation) return;
        // A failed durable write is surfaced, never silently dropped.
        this.#enqueueError = errorMessage(err);
        this.#retire([payload.id], []);
      },
    );
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
    this.#persistCache.flush();
    this.#unregisterCurrent();
    this.#generation += 1;
    this.#owner = owner;
    this.#syncedAt = null;
    this.#pendingBookmarks = new Map();
    this.#pendingFolders = new Map();
    this.#mutationEntityIds.clear();
    if (this.#firstSyncRetryTimer !== null) clearTimeout(this.#firstSyncRetryTimer);
    this.#firstSyncRetryTimer = null;
    this.#firstSyncRetries = 0;
    const last = readRaw("local", lastFolderKeyFor(owner));
    this.#lastFolder = last !== null && last.length > 0 ? last : null;
    const raw = owner === LOCAL_OWNER ? readJSON(LOCAL_KEY) : readJSON(cacheKeyFor(Number(owner)));
    const stored = decodeBookmarksSnapshot(raw);
    this.#folders = stored?.folders ?? [];
    this.#bookmarks = stored?.bookmarks ?? [];
    if (owner === LOCAL_OWNER && !this.#folders.some((f) => f.isDefault)) {
      this.#folders = [seedDefault(this.#defaultIdFor(owner)), ...this.#folders];
    }
  }

  #writeLocal(): void {
    writeJSON(LOCAL_KEY, { v: LOCAL_VERSION, folders: this.#folders, bookmarks: this.#bookmarks });
  }

  #writeCache(): void {
    const ownerId = this.#accountId();
    if (ownerId === null) return;
    writeJSON(cacheKeyFor(ownerId), { folders: this.#folders, bookmarks: this.#bookmarks });
  }

  /** Sign-in: device rows join the account; the device default maps onto the account default. */
  #absorb(local: BookmarksSnapshot): void {
    const localDefault = defaultFolderIdFor("local");
    const accountDefault = this.defaultFolderId;
    const remap = (id: string): string => (id === localDefault ? accountDefault : id);
    for (const folder of local.folders) {
      if (folder.isDefault) continue;
      this.#putFolder(folder);
    }
    // A renamed device default carries its name to the account default.
    const renamed = local.folders.find((f) => f.isDefault && f.name !== DEFAULT_FOLDER_NAME);
    if (renamed !== undefined)
      this.#putFolder({
        ...seedDefault(accountDefault),
        name: renamed.name,
        isDefault: true,
        updatedAt: renamed.updatedAt,
      });
    for (const row of local.bookmarks) this.#putBookmark({ ...row, folderId: remap(row.folderId) });
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
    const domain = createBookmarksDomain(this.#auth, this.#state, ownerId, isCurrent, () => {
      if (!isCurrent()) return;
      this.suspendAuth();
      this.#state.notifySessionLost?.();
    });
    const adapter: SyncDomain<BookmarksMutation, BookmarksSnapshot> = {
      name,
      sync: async (mutations, signal) => {
        try {
          return await domain.sync(mutations, signal);
        } catch (err) {
          if (err instanceof SyncPausedError) this.#scheduleFirstSyncRetry(ownerId, generation);
          throw err;
        }
      },
      applyServer: (state, drained) => {
        if (isCurrent()) this.applyServer(state, drained);
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

  /** Another tab drained these mutations from the shared outbox: retire our overlays and pull. */
  #installDrainListener(): void {
    if (this.#drainChannel !== null || !("BroadcastChannel" in globalThis)) return;
    const channel = new BroadcastChannel(SYNC_DRAIN_CHANNEL);
    channel.onmessage = (event: MessageEvent<unknown>): void => {
      const message = decodeDrainMessage(event.data);
      const ownerId = this.#accountId();
      if (message === null || ownerId === null || message.domain !== bookmarksDomainFor(ownerId)) {
        return;
      }
      const entityIds = message.ids.flatMap((id) => {
        const entity = this.#mutationEntityIds.get(id);
        return entity === undefined ? [] : [entity];
      });
      this.#retire(entityIds, message.ids);
      void this.#engine.hydrate?.();
      void this.#engine.flush(message.domain).catch(() => undefined);
    };
    this.#drainChannel = channel;
  }
}

export function createBookmarksStore(deps: BookmarksStoreDeps): BookmarksStore {
  return new BookmarksStore(deps);
}

export const bookmarks: BookmarksStore = createBookmarksStore({
  engine: syncEngine,
  auth: authClient,
});
