import { browser } from "$app/environment";
import { authClient } from "$lib/auth/auth-client";
import { authState } from "$lib/auth/auth-state.svelte";
import { reader } from "$lib/stores/reader.svelte";
import type { VerseKey } from "$lib/data/quran";
import { readRaw, writeRaw } from "$lib/storage/safe-storage";
import { registerDomain, syncEngine, type RegisteredSyncDomain, type SyncDomain, type SyncMutation, type SyncStatus } from "$lib/sync";

import { createBookmarksDomain, type BookmarkAuthLike } from "./domain";
import {
  FOLDER_NAME_MAX,
  newBookmarkEntityId,
  parseVerseKey,
  verseKeyOf,
  type Bookmark,
  type BookmarkFolder,
  type BookmarksMutation,
  type BookmarksSnapshot,
} from "./schema";

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SyncEngineLike {
  enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>>;
  clear(domain: string): Promise<void>;
  flush(domain?: string): Promise<void>;
  start(): () => void;
  readonly pending: number;
  readonly status: SyncStatus;
}

/** Legacy reader seam used for the anonymous view and login migration. */
export interface ReaderLike {
  isBookmarked(key: VerseKey): boolean;
  toggleBookmark(key: VerseKey): void;
  readonly bookmarkedKeys: readonly VerseKey[];
}

/** Auth status source shape; authState satisfies this structurally (user carries at least an id). */
export interface BookmarksAuthStateLike {
  readonly authenticated: boolean;
  readonly user: { readonly id: number } | null;
}

export interface BookmarksStoreDeps {
  readonly engine: SyncEngineLike;
  readonly auth: BookmarkAuthLike;
  readonly reader: ReaderLike;
  /** Auth status source; the default is the app singleton. */
  readonly state?: BookmarksAuthStateLike;
  /** Domain registration seam; the default is the app-wide registry. */
  readonly register?: (domain: RegisteredSyncDomain) => void;
}

/** Durable per-account "legacy bookmarks already migrated" marker (value "1"). */
function legacyMigratedKey(userId: string): string {
  return `eq.bookmarks.legacy-migrated.${userId}`;
}

function nowIso(): string {
  return new Date().toISOString();
}

function isValidFolderName(name: string): boolean {
  const trimmed = name.trim();
  return trimmed.length > 0 && trimmed.length <= FOLDER_NAME_MAX;
}

export class BookmarksStore {
  readonly #engine: SyncEngineLike;
  readonly #auth: BookmarkAuthLike;
  readonly #reader: ReaderLike;
  readonly #state: BookmarksAuthStateLike;
  readonly #register: (domain: RegisteredSyncDomain) => void;

  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  #hydrated = false;
  #syncActive = false;
  /** Legacy keys already migrated this tab session (in-session guard on top of the durable per-account marker). */
  #migratedKeys = new Set<string>();
  /** Optimistic upserts awaiting their drain ack, by bookmark entity id. */
  #pendingById = new Map<string, Bookmark>();
  /** Entity ids optimistically deleted, awaiting their drain ack. */
  #pendingDeletes = new Set<string>();

  constructor(deps: BookmarksStoreDeps) {
    this.#engine = deps.engine;
    this.#auth = deps.auth;
    this.#reader = deps.reader;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
  }

  get authed(): boolean {
    return this.#state.authenticated;
  }

  get folders(): readonly BookmarkFolder[] {
    return this.#folders;
  }

  get bookmarks(): readonly Bookmark[] {
    return this.#bookmarks;
  }

  get pending(): number {
    return this.#engine.pending;
  }

  get status(): SyncStatus {
    return this.#engine.status;
  }

  folderById(id: string): BookmarkFolder | undefined {
    return this.#folders.find((folder) => folder.id === id);
  }

  bookmarksIn(folderId: string | null): readonly Bookmark[] {
    return this.#bookmarks.filter((bookmark) => bookmark.folderId === folderId);
  }

  /**
   * Authed view grouping: every bookmark filed under its folder id, with rows
   * whose folderId references a missing folder (orphans) grouped under null —
   * no row ever disappears between groups.
   */
  bookmarksByFolder(): Map<string | null, readonly Bookmark[]> {
    const groups = new Map<string | null, Bookmark[]>();
    for (const bookmark of this.#bookmarks) {
      const folderId =
        bookmark.folderId !== null && this.#folders.some((folder) => folder.id === bookmark.folderId)
          ? bookmark.folderId
          : null;
      const rows = groups.get(folderId);
      if (rows === undefined) groups.set(folderId, [bookmark]);
      else rows.push(bookmark);
    }
    return groups;
  }

  /** Authed server view. */
  isBookmarked(surah: number, ayah: number): boolean {
    return this.#bookmarks.some((b) => b.surah === surah && b.ayah === ayah);
  }

  /** Legacy local view (anonymous users). */
  legacyBookmarked(key: VerseKey): boolean {
    return this.#reader.isBookmarked(key);
  }

  /** Unified view: authed → server state, anon → legacy local state. */
  isMarkedKey(key: string): boolean {
    const coords = parseVerseKey(key);
    if (coords === null) return this.#reader.isBookmarked(key);
    if (this.authed) return this.isBookmarked(coords.surah, coords.ayah);
    return this.#reader.isBookmarked(key);
  }

  /** Unified toggle: authed → optimistic + enqueued sync op, anon → legacy local toggle. */
  toggleVerse(key: string): void {
    const coords = parseVerseKey(key);
    if (coords === null) {
      this.#reader.toggleBookmark(key);
      return;
    }
    this.toggle(coords.surah, coords.ayah);
  }

  toggle(surah: number, ayah: number): void {
    if (!this.authed) {
      this.#reader.toggleBookmark(verseKeyOf(surah, ayah));
      return;
    }
    const existing = this.#bookmarks.find((b) => b.surah === surah && b.ayah === ayah);
    if (existing) {
      this.remove(existing.id);
      return;
    }
    const bookmark: Bookmark = {
      id: newBookmarkEntityId(),
      folderId: null,
      surah,
      ayah,
      createdAt: nowIso(),
      updatedAt: nowIso(),
    };
    this.#bookmarks = [...this.#bookmarks, bookmark];
    this.#enqueue({
      kind: "bookmark.upsert",
      id: bookmark.id,
      folderId: null,
      surah,
      ayah,
      updatedAt: bookmark.updatedAt,
    });
  }

  remove(bookmarkId: string): void {
    if (!this.authed) return;
    const existing = this.#bookmarks.find((b) => b.id === bookmarkId);
    if (!existing) return;
    this.#bookmarks = this.#bookmarks.filter((b) => b.id !== bookmarkId);
    this.#enqueue({ kind: "bookmark.delete", id: bookmarkId, updatedAt: nowIso() });
  }

  moveToFolder(bookmarkId: string, folderId: string | null): void {
    if (!this.authed) return;
    if (folderId !== null && !this.#folders.some((folder) => folder.id === folderId)) return;
    const existing = this.#bookmarks.find((b) => b.id === bookmarkId);
    if (!existing || existing.folderId === folderId) return;
    const updated: Bookmark = { ...existing, folderId, updatedAt: nowIso() };
    this.#bookmarks = this.#bookmarks.map((b) => (b.id === bookmarkId ? updated : b));
    this.#enqueue({
      kind: "bookmark.upsert",
      id: updated.id,
      folderId,
      surah: updated.surah,
      ayah: updated.ayah,
      updatedAt: updated.updatedAt,
    });
  }

  createFolder(name: string): BookmarkFolder | null {
    if (!this.authed || !isValidFolderName(name)) return null;
    const folder: BookmarkFolder = {
      id: newBookmarkEntityId(),
      name: name.trim(),
      createdAt: nowIso(),
      updatedAt: nowIso(),
    };
    this.#folders = [...this.#folders, folder];
    this.#enqueue({ kind: "folder.upsert", id: folder.id, name: folder.name, updatedAt: folder.updatedAt });
    return folder;
  }

  renameFolder(id: string, name: string): void {
    if (!this.authed || !isValidFolderName(name)) return;
    const existing = this.#folders.find((folder) => folder.id === id);
    if (!existing) return;
    const renamed: BookmarkFolder = { ...existing, name: name.trim(), updatedAt: nowIso() };
    this.#folders = this.#folders.map((folder) => (folder.id === id ? renamed : folder));
    this.#enqueue({ kind: "folder.upsert", id, name: renamed.name, updatedAt: renamed.updatedAt });
  }

  deleteFolder(id: string): void {
    if (!this.authed) return;
    if (!this.#folders.some((folder) => folder.id === id)) return;
    this.#folders = this.#folders.filter((folder) => folder.id !== id);
    // Server detaches children on folder.delete; mirror it optimistically.
    this.#bookmarks = this.#bookmarks.map((b) => (b.folderId === id ? { ...b, folderId: null } : b));
    this.#enqueue({ kind: "folder.delete", id, updatedAt: nowIso() });
  }

  /**
   * Replace local state from a decoded server snapshot. Rows referencing a
   * missing folder are kept (server order may race a folder.delete) and land in
   * the unfiled group in the view; duplicates per verse collapse to the newest
   * updatedAt so a cross-device migration never shows two rows for one verse.
   *
   * `drained` carries the mutations the engine just removed from the queue:
   * their optimistic overlay entries are retired first, so only still-in-flight
   * local edits survive the snapshot. The rest of the overlay is applied on top
   * — snapshot rows for pending-deleted entities are dropped, pending upserts
   * win their verse — so a stale snapshot can never flicker an optimistic
   * toggle away.
   *
   * No-op while signed out: a round that was mid-flight at logout must not
   * repopulate the old account's view after the logout reset.
   */
  applyServer(snapshot: BookmarksSnapshot, drained?: readonly SyncMutation<BookmarksMutation>[]): void {
    if (!this.authed) return;
    if (drained !== undefined) {
      for (const mutation of drained) {
        this.#pendingById.delete(mutation.payload.id);
        this.#pendingDeletes.delete(mutation.payload.id);
      }
    }
    const newestPerVerse = new Map<string, Bookmark>();
    for (const bookmark of snapshot.bookmarks) {
      if (this.#pendingDeletes.has(bookmark.id)) continue;
      const key = verseKeyOf(bookmark.surah, bookmark.ayah);
      const prev = newestPerVerse.get(key);
      if (prev === undefined || bookmark.updatedAt >= prev.updatedAt) newestPerVerse.set(key, bookmark);
    }
    for (const pending of this.#pendingById.values()) {
      newestPerVerse.set(verseKeyOf(pending.surah, pending.ayah), pending);
    }
    this.#folders = [...snapshot.folders];
    this.#bookmarks = [...newestPerVerse.values()];
  }

  /** Idempotent, browser-only boot: starts sync when a session already exists. */
  hydrate(): void {
    if (!browser || this.#hydrated) return;
    this.#hydrated = true;
    if (this.authed) void this.#beginSync();
  }

  /**
   * Auth transition edge, driven by an $effect over authState.authenticated in
   * the app layout. false→true: migrate legacy bookmarks + start syncing.
   * true→false: drop the server view (the anon view owns the screen again),
   * durably clear this domain's queue so nothing flushes into the next account,
   * and reset the in-session migration guard (the durable per-account marker
   * still gates re-login migration).
   */
  onAuthChanged(authed: boolean): void {
    if (authed) {
      void this.#beginSync();
      return;
    }
    this.#syncActive = false;
    this.#folders = [];
    this.#bookmarks = [];
    this.#migratedKeys.clear();
    this.#pendingById.clear();
    this.#pendingDeletes.clear();
    void this.#engine.clear("bookmarks").catch(() => undefined);
  }

  async #beginSync(): Promise<void> {
    if (this.#syncActive) return;
    this.#syncActive = true;
    await this.#migrateLegacy();
    const domain = createBookmarksDomain(this.#auth, this.#state);
    // Method-syntax members keep the domain bivariant, so the typed adapter
    // registers as the engine-erased RegisteredSyncDomain (see sync/types.ts).
    const adapter: SyncDomain<BookmarksMutation, BookmarksSnapshot> = {
      name: domain.name,
      sync: (mutations, signal) => domain.sync(mutations, signal),
      applyServer: (state, drained) => {
        this.applyServer(state, drained);
      },
    };
    this.#register(adapter);
    this.#engine.start();
    await this.#engine.flush("bookmarks");
  }

  /**
   * One-time-per-account import of legacy local bookmarks: enqueue an upsert
   * per verse (fresh uuids, folderId null), then write the durable per-account
   * marker — only after every enqueue resolved, so both are durable together.
   * The marker survives reloads and logout (device+account truth: this browser
   * already handed these keys to this account); the server's per-verse dedupe
   * makes any partial-failure re-run convergent. Legacy data on disk is never
   * deleted — the anon view keeps using it.
   */
  async #migrateLegacy(): Promise<void> {
    const user = this.#state.user;
    const markerKey = user !== null ? legacyMigratedKey(String(user.id)) : null;
    if (markerKey !== null && readRaw("local", markerKey) === "1") return;
    const payloads: BookmarksMutation[] = [];
    for (const key of this.#reader.bookmarkedKeys) {
      if (this.#migratedKeys.has(key)) continue;
      this.#migratedKeys.add(key);
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      payloads.push({
        kind: "bookmark.upsert",
        id: newBookmarkEntityId(),
        folderId: null,
        surah: coords.surah,
        ayah: coords.ayah,
        updatedAt: nowIso(),
      });
    }
    if (markerKey === null) return;
    if (payloads.length === 0) {
      writeRaw("local", markerKey, "1");
      return;
    }
    try {
      await Promise.all(payloads.map((payload) => this.#engine.enqueue("bookmarks", payload)));
      writeRaw("local", markerKey, "1");
    } catch {
      // Marker stays unwritten: the next session re-runs migration; the
      // server's per-verse dedupe keeps that harmless.
    }
  }

  #enqueue(payload: BookmarksMutation): void {
    if (payload.kind === "bookmark.delete") {
      this.#pendingDeletes.add(payload.id);
      this.#pendingById.delete(payload.id);
    } else if (payload.kind === "bookmark.upsert") {
      const row = this.#bookmarks.find((bookmark) => bookmark.id === payload.id);
      if (row !== undefined) this.#pendingById.set(payload.id, row);
    }
    // Enqueue persists before resolving; a storage failure would only surface as
    // a temporary optimistic drift, corrected by the next server snapshot.
    void this.#engine.enqueue("bookmarks", payload).catch(() => undefined);
  }
}

export function createBookmarksStore(deps: BookmarksStoreDeps): BookmarksStore {
  return new BookmarksStore(deps);
}

export const bookmarks: BookmarksStore = createBookmarksStore({
  engine: syncEngine,
  auth: authClient,
  reader,
});
