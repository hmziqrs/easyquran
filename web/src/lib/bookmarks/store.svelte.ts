import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import type { VerseKey } from "#lib/data/quran.js";
import { readRaw, writeRaw } from "#lib/storage/safe-storage.js";
import { reader } from "#lib/stores/reader.svelte.js";
import {
  registerDomain,
  unregisterDomain,
  syncEngine,
  type RegisteredSyncDomain,
  type SyncDomain,
  type SyncMutation,
  type SyncStatus,
} from "#lib/sync/index.js";

import {
  BOOKMARKS_DOMAIN,
  bookmarksDomainFor,
  createBookmarksDomain,
  type BookmarkAuthLike,
} from "./domain";
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
  pendingFor(domain: string): number;
  flush(domain?: string): Promise<void>;
  start(): () => void;
  readonly pending: number;
  readonly status: SyncStatus;
}

/** Legacy reader seam used for the anonymous view and login migration. */
export interface ReaderLike {
  hydrate?(): void;
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
  readonly state?: BookmarksAuthStateLike;
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

function legacyMigratedKey(userId: string): string {
  return `eq.bookmarks.legacy-migrated.${userId}`;
}

interface AccountGeneration {
  readonly ownerId: number;
  readonly generation: number;
  readonly domain: string;
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
  readonly #unregister: (name: string) => void;

  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  #hydrated = false;
  #syncGeneration: number | null = null;
  #ownerId: number | null;
  #generation = 0;
  #suspended = $state(false);
  #legacyDiscard: Promise<void> | null = null;
  #migratedKeys = new Set<string>();
  #pendingById = new Map<string, Bookmark>();
  #pendingDeletes = new Set<string>();
  #pendingFoldersById = new Map<string, BookmarkFolder>();
  #pendingFolderDeletes = new Set<string>();

  constructor(deps: BookmarksStoreDeps) {
    this.#engine = deps.engine;
    this.#auth = deps.auth;
    this.#reader = deps.reader;
    this.#state = deps.state ?? authState;
    this.#register = deps.register ?? registerDomain;
    this.#unregister = deps.unregister ?? unregisterDomain;
    this.#ownerId = this.#currentOwnerId();
  }

  get authed(): boolean {
    return !this.#suspended && this.#currentOwnerId() !== null;
  }

  get folders(): readonly BookmarkFolder[] {
    if (!this.#viewIsCurrent()) return [];
    return this.#folders;
  }

  get bookmarks(): readonly Bookmark[] {
    if (!this.#viewIsCurrent()) return [];
    return this.#bookmarks;
  }

  get pending(): number {
    const ownerId = this.#currentOwnerId();
    if (ownerId === null || this.#suspended) return 0;
    return this.#engine.pendingFor(bookmarksDomainFor(ownerId));
  }

  get status(): SyncStatus {
    return { ...this.#engine.status, pending: this.pending };
  }

  folderById(id: string): BookmarkFolder | undefined {
    return this.folders.find((folder) => folder.id === id);
  }

  bookmarksIn(folderId: string | null): readonly Bookmark[] {
    return this.bookmarks.filter((bookmark) => bookmark.folderId === folderId);
  }

  bookmarksByFolder(): Map<string | null, readonly Bookmark[]> {
    const groups = new Map<string | null, Bookmark[]>();
    for (const bookmark of this.bookmarks) {
      const folderId =
        bookmark.folderId !== null &&
        this.#folders.some((folder) => folder.id === bookmark.folderId)
          ? bookmark.folderId
          : null;
      const rows = groups.get(folderId);
      if (rows === undefined) groups.set(folderId, [bookmark]);
      else rows.push(bookmark);
    }
    return groups;
  }

  #effectiveRow(surah: number, ayah: number): Bookmark | undefined {
    if (!this.#viewIsCurrent()) return undefined;
    for (const pending of this.#pendingById.values()) {
      if (pending.surah === surah && pending.ayah === ayah) return pending;
    }
    return this.#bookmarks.find((b) => b.surah === surah && b.ayah === ayah);
  }

  isBookmarked(surah: number, ayah: number): boolean {
    return this.#effectiveRow(surah, ayah) !== undefined;
  }

  legacyBookmarked(key: VerseKey): boolean {
    return this.#reader.isBookmarked(key);
  }

  isMarkedKey(key: string): boolean {
    const coords = parseVerseKey(key);
    if (coords === null) return this.#reader.isBookmarked(key);
    if (this.authed) return this.isBookmarked(coords.surah, coords.ayah);
    return this.#reader.isBookmarked(key);
  }

  toggleVerse(key: string): void {
    const coords = parseVerseKey(key);
    if (coords === null) {
      this.#reader.toggleBookmark(key);
      return;
    }
    this.toggle(coords.surah, coords.ayah);
  }

  toggle(surah: number, ayah: number): void {
    if (this.#suspended) return;
    if (this.#captureAccount() === null) {
      this.#reader.toggleBookmark(verseKeyOf(surah, ayah));
      return;
    }
    const existing = this.#effectiveRow(surah, ayah);
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
        if (this.#bookmarks.some((b) => b.surah === surah && b.ayah === ayah)) return;
        this.#bookmarks = [...this.#bookmarks, bookmark];
      },
      { bookmark },
    );
  }

  remove(bookmarkId: string): void {
    if (this.#captureAccount() === null) return;
    const durable = this.#bookmarks.find((b) => b.id === bookmarkId);
    if (durable === undefined && !this.#pendingById.has(bookmarkId)) return;
    this.#enqueue({ kind: "bookmark.delete", id: bookmarkId, updatedAt: nowIso() }, () => {
      this.#bookmarks = this.#bookmarks.filter((b) => b.id !== bookmarkId);
    });
  }

  moveToFolder(bookmarkId: string, folderId: string | null): void {
    if (this.#captureAccount() === null) return;
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
    if (this.#captureAccount() === null || !isValidFolderName(name)) return null;
    const folder: BookmarkFolder = {
      id: newBookmarkEntityId(),
      name: name.trim(),
      createdAt: nowIso(),
      updatedAt: nowIso(),
    };
    this.#enqueue(
      { kind: "folder.upsert", id: folder.id, name: folder.name, updatedAt: folder.updatedAt },
      () => {
        if (this.#folders.some((existing) => existing.id === folder.id)) return;
        this.#folders = [...this.#folders, folder];
      },
      { folder },
    );
    return folder;
  }

  renameFolder(id: string, name: string): void {
    if (this.#captureAccount() === null || !isValidFolderName(name)) return;
    const existing = this.#folders.find((folder) => folder.id === id);
    if (!existing) return;
    const renamed: BookmarkFolder = { ...existing, name: name.trim(), updatedAt: nowIso() };
    this.#enqueue(
      { kind: "folder.upsert", id, name: renamed.name, updatedAt: renamed.updatedAt },
      () => {
        this.#folders = this.#folders.map((folder) => (folder.id === id ? renamed : folder));
      },
      { folder: renamed },
    );
  }

  deleteFolder(id: string): void {
    if (this.#captureAccount() === null) return;
    if (!this.#folders.some((folder) => folder.id === id)) return;
    this.#enqueue({ kind: "folder.delete", id, updatedAt: nowIso() }, () => {
      this.#folders = this.#folders.filter((folder) => folder.id !== id);
      this.#bookmarks = this.#bookmarks.map((b) =>
        b.folderId === id ? { ...b, folderId: null } : b,
      );
    });
  }

  applyServer(
    snapshot: BookmarksSnapshot,
    drained?: readonly SyncMutation<BookmarksMutation>[],
  ): void {
    if (!this.#viewIsCurrent()) return;
    if (drained !== undefined) {
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
      if (prev === undefined || bookmark.updatedAt >= prev.updatedAt)
        newestPerVerse.set(key, bookmark);
    }
    for (const pending of this.#pendingById.values()) {
      newestPerVerse.set(verseKeyOf(pending.surah, pending.ayah), pending);
    }
    this.#folders = [
      ...snapshot.folders.filter(
        (folder) =>
          !this.#pendingFolderDeletes.has(folder.id) && !this.#pendingFoldersById.has(folder.id),
      ),
      ...this.#pendingFoldersById.values(),
    ];
    this.#bookmarks = [...newestPerVerse.values()];
  }

  hydrate(): void {
    if (!browser || this.#hydrated) return;
    this.#hydrated = true;
    void this.#discardUnownedQueue().catch(() => undefined);
    this.onAuthChanged(this.#state.authenticated);
  }

  onAuthChanged(authed: boolean): void {
    this.#suspended = false;
    const nextOwnerId = authed ? this.#currentOwnerId() : null;
    if (nextOwnerId !== this.#ownerId) this.#resetAccount(nextOwnerId);
    const account = this.#captureAccount();
    if (account === null) return;
    void this.#beginSync(account).catch(() => {
      if (this.#isCurrent(account)) this.#syncGeneration = null;
    });
  }

  suspendAuth(): void {
    this.#suspended = true;
    this.#resetAccount(null);
  }

  #currentOwnerId(): number | null {
    if (!this.#state.authenticated) return null;
    return this.#state.user?.id ?? null;
  }

  #viewIsCurrent(): boolean {
    return !this.#suspended && this.#ownerId !== null && this.#ownerId === this.#currentOwnerId();
  }

  #captureAccount(): AccountGeneration | null {
    if (this.#suspended) return null;
    const ownerId = this.#currentOwnerId();
    if (ownerId !== this.#ownerId) this.#resetAccount(ownerId);
    if (ownerId === null) return null;
    return { ownerId, generation: this.#generation, domain: bookmarksDomainFor(ownerId) };
  }

  #isCurrent(account: AccountGeneration): boolean {
    return (
      this.#viewIsCurrent() &&
      this.#ownerId === account.ownerId &&
      this.#generation === account.generation
    );
  }

  #resetAccount(ownerId: number | null): void {
    if (this.#ownerId !== null) this.#unregister(bookmarksDomainFor(this.#ownerId));
    this.#generation += 1;
    this.#ownerId = ownerId;
    this.#syncGeneration = null;
    this.#folders = [];
    this.#bookmarks = [];
    this.#migratedKeys.clear();
    this.#pendingById.clear();
    this.#pendingDeletes.clear();
    this.#pendingFoldersById.clear();
    this.#pendingFolderDeletes.clear();
  }

  #discardUnownedQueue(): Promise<void> {
    if (this.#legacyDiscard !== null) return this.#legacyDiscard;
    const discard = this.#engine.clear(BOOKMARKS_DOMAIN);
    this.#legacyDiscard = discard;
    void discard.catch(() => {
      if (this.#legacyDiscard === discard) this.#legacyDiscard = null;
    });
    return discard;
  }

  async #beginSync(account: AccountGeneration): Promise<void> {
    if (this.#syncGeneration === account.generation) return;
    this.#syncGeneration = account.generation;
    await this.#discardUnownedQueue();
    if (!this.#isCurrent(account)) return;
    await this.#migrateLegacy(account);
    if (!this.#isCurrent(account)) return;
    const domain = createBookmarksDomain(this.#auth, this.#state, account.ownerId, () =>
      this.#isCurrent(account),
    );
    const adapter: SyncDomain<BookmarksMutation, BookmarksSnapshot> = {
      name: domain.name,
      sync: (mutations, signal) => domain.sync(mutations, signal),
      applyServer: (state, drained) => {
        if (this.#isCurrent(account)) this.applyServer(state, drained);
      },
    };
    this.#register(adapter);
    this.#engine.start();
    await this.#engine.flush(account.domain);
  }

  async #migrateLegacy(account: AccountGeneration): Promise<void> {
    const markerKey = legacyMigratedKey(String(account.ownerId));
    if (readRaw("local", markerKey) === "1") return;
    this.#reader.hydrate?.();
    for (const key of this.#reader.bookmarkedKeys) {
      if (!this.#isCurrent(account)) return;
      if (this.#migratedKeys.has(key)) continue;
      const coords = parseVerseKey(key);
      if (coords === null) continue;
      try {
        await this.#engine.enqueue(account.domain, {
          kind: "bookmark.upsert",
          id: newBookmarkEntityId(),
          folderId: null,
          surah: coords.surah,
          ayah: coords.ayah,
          updatedAt: nowIso(),
        } satisfies BookmarksMutation);
      } catch {
        return;
      }
      if (!this.#isCurrent(account)) return;
      this.#migratedKeys.add(key);
    }
    if (this.#isCurrent(account)) writeRaw("local", markerKey, "1");
  }

  #enqueue(
    payload: BookmarksMutation,
    apply: () => void,
    overlay?: { bookmark?: Bookmark; folder?: BookmarkFolder },
  ): void {
    const account = this.#captureAccount();
    if (account === null) return;
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
    void this.#engine.enqueue(account.domain, payload).then(
      () => {
        if (this.#isCurrent(account)) apply();
      },
      () => {
        if (!this.#isCurrent(account)) return;
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
