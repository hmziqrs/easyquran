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
  /** Optimistic folder upserts awaiting their drain ack, by folder entity id. */
  #pendingFoldersById = new Map<string, BookmarkFolder>();
  /** Folder entity ids optimistically deleted, awaiting their drain ack. */
  #pendingFolderDeletes = new Set<string>();
  /**
   * Last authed edge seen (null = no edge yet). AuthState boots "unknown"
   * with authenticated === false, so a false edge before any true edge is the
   * probe not having landed — never a logout.
   */
  #lastAuthed: boolean | null = null;

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
    // Consult the pending overlay too: a double-fire before the optimistic row
    // lands must read as "on", so the second fire cancels the same entity
    // instead of minting a second upsert that the server would re-add.
    const pending = [...this.#pendingById.values()].find((b) => b.surah === surah && b.ayah === ayah);
    const existing = pending ?? this.#bookmarks.find((b) => b.surah === surah && b.ayah === ayah);
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
    this.#enqueue(
      {
        kind: "bookmark.upsert",
        id: bookmark.id,
        folderId: null,
        surah,
        ayah,
        updatedAt: bookmark.updatedAt,
      },
      () => {
        // A snapshot (or a rapid re-toggle) may have landed the verse already;
        // the store keeps one row per verse.
        if (this.isBookmarked(surah, ayah)) return;
        this.#bookmarks = [...this.#bookmarks, bookmark];
      },
      { bookmark },
    );
  }

  remove(bookmarkId: string): void {
    if (!this.authed) return;
    // A durable-but-unapplied row lives only in the pending overlay
    // (#pendingById has it, #bookmarks does not): still enqueue the
    // compensating bookmark.delete for that pending row's id — #enqueue's
    // delete branch retires the overlay entry — instead of no-op'ing.
    const durable = this.#bookmarks.find((b) => b.id === bookmarkId);
    if (durable === undefined && !this.#pendingById.has(bookmarkId)) return;
    this.#enqueue({ kind: "bookmark.delete", id: bookmarkId, updatedAt: nowIso() }, () => {
      this.#bookmarks = this.#bookmarks.filter((b) => b.id !== bookmarkId);
    });
  }

  moveToFolder(bookmarkId: string, folderId: string | null): void {
    if (!this.authed) return;
    if (folderId !== null && !this.#folders.some((folder) => folder.id === folderId)) return;
    const existing = this.#bookmarks.find((b) => b.id === bookmarkId);
    if (!existing || existing.folderId === folderId) return;
    const updated: Bookmark = { ...existing, folderId, updatedAt: nowIso() };
    this.#enqueue(
      {
        kind: "bookmark.upsert",
        id: updated.id,
        folderId,
        surah: updated.surah,
        ayah: updated.ayah,
        updatedAt: updated.updatedAt,
      },
      () => {
        this.#bookmarks = this.#bookmarks.map((b) => (b.id === bookmarkId ? updated : b));
      },
      { bookmark: updated },
    );
  }

  createFolder(name: string): BookmarkFolder | null {
    if (!this.authed || !isValidFolderName(name)) return null;
    const folder: BookmarkFolder = {
      id: newBookmarkEntityId(),
      name: name.trim(),
      createdAt: nowIso(),
      updatedAt: nowIso(),
    };
    this.#enqueue({ kind: "folder.upsert", id: folder.id, name: folder.name, updatedAt: folder.updatedAt }, () => {
      if (this.#folders.some((existing) => existing.id === folder.id)) return;
      this.#folders = [...this.#folders, folder];
    }, { folder });
    return folder;
  }

  renameFolder(id: string, name: string): void {
    if (!this.authed || !isValidFolderName(name)) return;
    const existing = this.#folders.find((folder) => folder.id === id);
    if (!existing) return;
    const renamed: BookmarkFolder = { ...existing, name: name.trim(), updatedAt: nowIso() };
    this.#enqueue({ kind: "folder.upsert", id, name: renamed.name, updatedAt: renamed.updatedAt }, () => {
      this.#folders = this.#folders.map((folder) => (folder.id === id ? renamed : folder));
    }, { folder: renamed });
  }

  deleteFolder(id: string): void {
    if (!this.authed) return;
    if (!this.#folders.some((folder) => folder.id === id)) return;
    this.#enqueue({ kind: "folder.delete", id, updatedAt: nowIso() }, () => {
      this.#folders = this.#folders.filter((folder) => folder.id !== id);
      // Server detaches children on folder.delete; mirror it.
      this.#bookmarks = this.#bookmarks.map((b) => (b.folderId === id ? { ...b, folderId: null } : b));
    });
  }

  /**
   * Replace local state from a decoded server snapshot. Rows referencing a
   * missing folder are kept (server order may race a folder.delete) and land in
   * the unfiled group in the view; duplicates per verse collapse to the newest
   * updatedAt so a cross-device migration never shows two rows for one verse.
   *
   * `drained` carries the mutations the engine just removed from the queue:
   * their optimistic overlay entries (bookmarks and folders alike) are
   * retired first, so only still-in-flight local edits survive the snapshot.
   * The rest of the overlay is applied on top — snapshot rows for
   * pending-deleted entities are dropped, pending upserts win their verse (or,
   * for folders, their id) — so a stale snapshot can never flicker an
   * optimistic toggle or folder edit away.
   *
   * No-op while signed out: a round that was mid-flight at logout must not
   * repopulate the old account's view after the logout reset.
   */
  applyServer(snapshot: BookmarksSnapshot, drained?: readonly SyncMutation<BookmarksMutation>[]): void {
    if (!this.authed) return;
    if (drained !== undefined) {
      // DEFERRED: a drain in another tab does not retire this tab's overlay
      // entries (no cross-tab retire signal exists yet).
      for (const mutation of drained) {
        this.#pendingById.delete(mutation.payload.id);
        this.#pendingDeletes.delete(mutation.payload.id);
        this.#pendingFoldersById.delete(mutation.payload.id);
        this.#pendingFolderDeletes.delete(mutation.payload.id);
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
    // Folder mirror of the bookmark overlay: a pull-only round whose snapshot
    // predates a local create/rename/delete must not erase or resurrect it.
    this.#folders = [
      ...snapshot.folders.filter(
        (folder) => !this.#pendingFolderDeletes.has(folder.id) && !this.#pendingFoldersById.has(folder.id),
      ),
      ...this.#pendingFoldersById.values(),
    ];
    this.#bookmarks = [...newestPerVerse.values()];
  }

  /** Idempotent, browser-only boot: starts sync when a session already exists. */
  hydrate(): void {
    if (!browser || this.#hydrated) return;
    this.#hydrated = true;
    if (this.authed) void this.#beginSync();
  }

  /**
   * Auth transition edge, driven by an $effect over authState in the app
   * layout (which no-ops while the probe status is "unknown"). false→true:
   * migrate legacy bookmarks + start syncing. true→false: drop the server view
   * (the anon view owns the screen again), durably clear this domain's queue
   * so nothing flushes into the next account, and reset the in-session
   * migration guard (the durable per-account marker still gates re-login
   * migration).
   *
   * Belt and braces for the layout's unknown-status guard: the logout path
   * only runs on a true authenticated→unauthenticated transition. A false
   * edge before any true edge is the boot-time "unknown" status resolving,
   * not a logout — running the logout path there would wipe the durable outbox
   * of an authed user on every cold load.
   */
  onAuthChanged(authed: boolean): void {
    if (authed) {
      void this.#beginSync();
      return;
    }
    if (this.#lastAuthed !== true) return;
    this.#lastAuthed = false;
    this.#syncActive = false;
    this.#folders = [];
    this.#bookmarks = [];
    this.#migratedKeys.clear();
    this.#pendingById.clear();
    this.#pendingDeletes.clear();
    this.#pendingFoldersById.clear();
    this.#pendingFolderDeletes.clear();
    void this.#engine.clear("bookmarks").catch(() => undefined);
  }

  async #beginSync(): Promise<void> {
    this.#lastAuthed = true;
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

  /**
   * Queue a mutation, then apply its optimistic view ONLY once the queue write
   * is durable — engine.enqueue resolves after the outbox write, so the view
   * can never run ahead of durability (a reload right after a toggle replays a
   * queued mutation, never a lost one). A rejected enqueue (storage failure)
   * applies nothing and retires the overlay entry, so view and queue stay in
   * step; a logout before resolution drops the apply along with the queue.
   */
  #enqueue(
    payload: BookmarksMutation,
    apply: () => void,
    overlay?: { bookmark?: Bookmark; folder?: BookmarkFolder },
  ): void {
    if (payload.kind === "bookmark.delete") {
      this.#pendingDeletes.add(payload.id);
      this.#pendingById.delete(payload.id);
    } else if (payload.kind === "folder.delete") {
      this.#pendingFolderDeletes.add(payload.id);
      this.#pendingFoldersById.delete(payload.id);
    } else if (payload.kind === "bookmark.upsert" && overlay?.bookmark !== undefined) {
      this.#pendingById.set(payload.id, overlay.bookmark);
    } else if (payload.kind === "folder.upsert" && overlay?.folder !== undefined) {
      this.#pendingFoldersById.set(payload.id, overlay.folder);
      this.#pendingFolderDeletes.delete(payload.id);
    }
    void this.#engine.enqueue("bookmarks", payload).then(
      () => {
        if (this.authed) apply();
      },
      () => {
        this.#pendingById.delete(payload.id);
        this.#pendingDeletes.delete(payload.id);
        this.#pendingFoldersById.delete(payload.id);
        this.#pendingFolderDeletes.delete(payload.id);
      },
    );
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
