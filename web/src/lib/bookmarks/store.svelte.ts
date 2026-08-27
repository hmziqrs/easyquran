import { browser } from "$app/environment";
import { authClient } from "$lib/auth/auth-client";
import { authState } from "$lib/auth/auth-state.svelte";
import { reader } from "$lib/stores/reader.svelte";
import type { VerseKey } from "$lib/data/quran";
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

export interface BookmarksStoreDeps {
  readonly engine: SyncEngineLike;
  readonly auth: BookmarkAuthLike;
  readonly reader: ReaderLike;
  /** Auth status source; the default is the app singleton. */
  readonly state?: { readonly authenticated: boolean };
  /** Domain registration seam; the default is the app-wide registry. */
  readonly register?: (domain: RegisteredSyncDomain) => void;
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
  readonly #state: { readonly authenticated: boolean };
  readonly #register: (domain: RegisteredSyncDomain) => void;

  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  #hydrated = false;
  #syncActive = false;
  /** Legacy keys already migrated this tab session; prevents server duplicates on re-login. */
  #migratedKeys = new Set<string>();

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
   */
  applyServer(snapshot: BookmarksSnapshot): void {
    const newestPerVerse = new Map<string, Bookmark>();
    for (const bookmark of snapshot.bookmarks) {
      const key = verseKeyOf(bookmark.surah, bookmark.ayah);
      const prev = newestPerVerse.get(key);
      if (prev === undefined || bookmark.updatedAt >= prev.updatedAt) newestPerVerse.set(key, bookmark);
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
   * true→false: drop the server view (the anon view owns the screen again);
   * queued-but-unsent mutations stay in the durable outbox.
   */
  onAuthChanged(authed: boolean): void {
    if (authed) {
      void this.#beginSync();
      return;
    }
    this.#syncActive = false;
    this.#folders = [];
    this.#bookmarks = [];
  }

  async #beginSync(): Promise<void> {
    if (this.#syncActive) return;
    this.#syncActive = true;
    this.#migrateLegacy();
    const domain = createBookmarksDomain(this.#auth);
    // Method-syntax members keep the domain bivariant, so the typed adapter
    // registers as the engine-erased RegisteredSyncDomain (see sync/types.ts).
    const adapter: SyncDomain<BookmarksMutation, BookmarksSnapshot> = {
      name: domain.name,
      sync: (mutations, signal) => domain.sync(mutations, signal),
      applyServer: (state) => {
        this.applyServer(state);
      },
    };
    this.#register(adapter);
    this.#engine.start();
    await this.#engine.flush("bookmarks");
  }

  /**
   * One-time-per-key import of legacy local bookmarks into the account: enqueue
   * an upsert per verse (fresh uuids, folderId null). Legacy data on disk is
   * never deleted — the anon view keeps using it.
   */
  #migrateLegacy(): void {
    for (const key of this.#reader.bookmarkedKeys) {
      if (this.#migratedKeys.has(key)) continue;
      this.#migratedKeys.add(key);
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      this.#enqueue({
        kind: "bookmark.upsert",
        id: newBookmarkEntityId(),
        folderId: null,
        surah: coords.surah,
        ayah: coords.ayah,
        updatedAt: nowIso(),
      });
    }
  }

  #enqueue(payload: BookmarksMutation): void {
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
