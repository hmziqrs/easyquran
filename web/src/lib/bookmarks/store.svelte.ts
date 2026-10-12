import { browser } from "$app/env";

import { authClient } from "#lib/auth/auth-client.js";
import { authState } from "#lib/auth/auth-state.svelte.js";
import type { VerseKey } from "#lib/data/quran.js";
import { reader } from "#lib/stores/reader.svelte.js";
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

import {
  BOOKMARKS_DOMAIN,
  bookmarksDomainFor,
  createBookmarksDomain,
  type BookmarkAuthLike,
} from "./domain";
import {
  FOLDER_NAME_MAX,
  bookmarkRangeKeyOf,
  isSingleVerseBookmark,
  newBookmarkEntityId,
  parseVerseKey,
  verseKeyOf,
  type Bookmark,
  type BookmarkFolder,
  type BookmarkFolderKind,
  type BookmarksMutation,
  type BookmarksSnapshot,
} from "./schema";

/** Engine surface the store depends on; the real SyncEngine satisfies it structurally. */
export interface SyncEngineLike {
  enqueue<P>(domain: string, payload: P): Promise<SyncMutation<P>>;
  clear(domain: string): Promise<void>;
  reassignDomain(from: string, to: string): Promise<number>;
  pendingFor(domain: string): number;
  flush(domain?: string): Promise<void>;
  start(): () => void;
  hydrate?(): Promise<void>;
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
  /** Session-loss seam (EC-15/16/19): probe confirms; anonymous ⇒ terminate transition + banner. */
  notifySessionLost?(): void;
}

export interface BookmarksStoreDeps {
  readonly engine: SyncEngineLike;
  readonly auth: BookmarkAuthLike;
  readonly reader: ReaderLike;
  readonly state?: BookmarksAuthStateLike;
  readonly register?: (domain: RegisteredSyncDomain) => void;
  readonly unregister?: (name: string) => void;
}

/**
 * Per-account marker written once the merge review (§9.4) has uploaded this
 * account's legacy blob bookmarks — gates merge-review detection condition (a).
 */
export function legacyMigratedKey(userId: string): string {
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

// eslint-disable-next-line anti-slop/no-unknown-parameters -- err is a caught throw of unknown provenance; this formatter is the boundary that narrows it to a displayable message string.
function errorMessage(err: unknown): string {
  return err instanceof Error && err.message ? err.message : String(err);
}

/** Narrow an untrusted cross-tab payload (§9.6): a drain notice is `{domain, ids[]}` and nothing else. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- data is the raw BroadcastChannel message payload; this decoder is its boundary parser.
function decodeDrainMessage(data: unknown): SyncDrainMessage | null {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- typeof-object is the only discriminator for the raw BroadcastChannel payload at this parse boundary.
  if (typeof data !== "object" || data === null) return null;
  if (!("domain" in data) || !("ids" in data)) return null;
  // SAFETY: property existence is checked above; this narrows the fields for validation only.
  const { domain, ids, page } = data as { domain: unknown; ids: unknown; page?: unknown };
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- domain is a wire string field with no static type before this check.
  if (typeof domain !== "string" || !Array.isArray(ids)) return null;
  // Same-page notices (this page's own engine published) are not cross-tab
  // retire triggers — the store retires its own overlays via applyServer.
  if (page === SYNC_PAGE_NONCE) return null;
  for (const id of ids) {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- each ids element is an opaque wire value until this string check.
    if (typeof id !== "string") return null;
  }
  return { domain, ids };
}

function isValidFolderName(name: string): boolean {
  const trimmed = name.trim();
  return trimmed.length > 0 && trimmed.length <= FOLDER_NAME_MAX;
}

/**
 * Paused-first-round backstop (round-2 false-empty fix): the engine treats a
 * paused round as no-penalty and schedules nothing, so the store retries its
 * own first pull a bounded number of times before the 30s periodic trigger.
 */
const FIRST_SYNC_RETRY_MS = 2_000;
const FIRST_SYNC_RETRY_MAX = 3;

export class BookmarksStore {
  readonly #engine: SyncEngineLike;
  readonly #auth: BookmarkAuthLike;
  readonly #reader: ReaderLike;
  readonly #state: BookmarksAuthStateLike;
  readonly #register: (domain: RegisteredSyncDomain) => void;
  readonly #unregister: (name: string) => void;

  #folders = $state.raw<readonly BookmarkFolder[]>([]);
  #bookmarks = $state.raw<readonly Bookmark[]>([]);
  /**
   * Per-domain sync truth (round-2 false-empty fix): epoch ms of the last
   * server state that actually landed for THIS account's domain, or null
   * before the first successful round. The engine's `lastSyncAt` is global —
   * any domain's success sets it — so the first-sync guard must not read it.
   */
  #syncedAt = $state<number | null>(null);
  #hydrated = false;
  #syncGeneration: number | null = null;
  #ownerId: number | null;
  #generation = 0;
  #suspended = $state(false);
  #enqueueError = $state<string | null>(null);
  #pendingById = new Map<string, Bookmark>();
  /**
   * In-flight `bookmark.delete` overlays: entity id → range key (start verse +
   * range end, `bookmarkRangeKeyOf`). Key-scoped so a stale same-key rival row
   * cannot re-mark a pending delete (EC-40); the key carries the range end
   * since range bookmarks landed (§9.1/EC-51).
   */
  #pendingDeletes = new Map<string, string>();
  #pendingFoldersById = new Map<string, BookmarkFolder>();
  #pendingFolderDeletes = new Set<string>();
  /**
   * Outbox mutation id → payload entity id. The §9.6 drain broadcast carries
   * mutation ids only (never user content), while the overlay maps are keyed by
   * entity id — this is the join between them.
   */
  #mutationEntityIds = new Map<string, string>();
  /** §9.6 cross-tab drain listener (EC-33); null until hydrate() in a browser. */
  #drainChannel: BroadcastChannel | null = null;
  /** Paused-first-round backstop timer (see FIRST_SYNC_RETRY_*); cleared on any account edge. */
  #firstSyncRetryTimer: ReturnType<typeof setTimeout> | null = null;
  #firstSyncRetries = 0;

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
    // Read the reactive list even on the stale path: a page derived that first
    // evaluated while the view was not current must still depend on #folders,
    // or a later applyServer write never invalidates it (false-empty fix).
    const view = this.#viewIsCurrent();
    const all = this.#folders;
    return view ? all : [];
  }

  get bookmarks(): readonly Bookmark[] {
    // Same read-through as `folders`: no early return that skips the reactive
    // state, so deriveds always subscribe to applyServer writes.
    const view = this.#viewIsCurrent();
    const all = this.#bookmarks;
    return view ? all : [];
  }

  get pending(): number {
    const ownerId = this.#currentOwnerId();
    if (ownerId === null || this.#suspended) return 0;
    return this.#engine.pendingFor(bookmarksDomainFor(ownerId));
  }

  get status(): SyncStatus {
    return { ...this.#engine.status, pending: this.pending };
  }

  /**
   * Sync truth for THIS store's account domain (round-2): set only when a
   * server snapshot actually landed on the live view, null before that. The
   * first-sync guard and the SyncIndicator read this — never the engine-global
   * `status.lastSyncAt`.
   */
  get syncedAt(): number | null {
    return this.#syncedAt;
  }

  /**
   * Last durable-enqueue failure (EC-27): surfaced for the indicator/toast
   * instead of silently dropping the mutation the user just acted on.
   */
  get enqueueError(): string | null {
    return this.#enqueueError;
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
    // Read-through like `bookmarks`: verse-mark readers must subscribe to
    // applyServer writes even when this evaluation started pre-auth.
    const view = this.#viewIsCurrent();
    const rows = this.#bookmarks;
    if (!view) return undefined;
    // Single rows only (EC-51): a range must not light the per-verse mark at
    // its start verse nor be deleted by a per-verse toggle there.
    for (const pending of this.#pendingById.values()) {
      if (isSingleVerseBookmark(pending) && pending.surah === surah && pending.ayah === ayah) {
        return pending;
      }
    }
    return rows.find((b) => isSingleVerseBookmark(b) && b.surah === surah && b.ayah === ayah);
  }

  /** Live row covering exactly this range (pending overlay first), or undefined. */
  #effectiveRangeRow(
    surah: number,
    ayah: number,
    endSurah: number,
    endAyah: number,
  ): Bookmark | undefined {
    if (!this.#viewIsCurrent()) return undefined;
    const covers = (b: Bookmark): boolean =>
      b.surah === surah &&
      b.ayah === ayah &&
      (b.endSurah ?? b.surah) === endSurah &&
      (b.endAyah ?? b.ayah) === endAyah;
    for (const pending of this.#pendingById.values()) {
      if (covers(pending)) return pending;
    }
    return this.#bookmarks.find(covers);
  }

  isRangeMarked(surah: number, ayah: number, endSurah: number, endAyah: number): boolean {
    return this.#effectiveRangeRow(surah, ayah, endSurah, endAyah) !== undefined;
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
        // Single rows only: a same-verse range row must not block the single.
        if (
          this.#bookmarks.some(
            (b) => isSingleVerseBookmark(b) && b.surah === surah && b.ayah === ayah,
          )
        ) {
          return;
        }
        this.#bookmarks = [...this.#bookmarks, bookmark];
      },
      { bookmark },
    );
  }

  /** Range bookmark (7.4/7.5): toggle `{surah}:{ayah}–{end}:{endAyah}`; anonymous → legacy single. */
  toggleRange(surah: number, ayah: number, endSurah: number, endAyah: number): void {
    if (this.#suspended) return;
    if (this.#captureAccount() === null) {
      this.#reader.toggleBookmark(verseKeyOf(surah, ayah));
      return;
    }
    const existing = this.#effectiveRangeRow(surah, ayah, endSurah, endAyah);
    if (existing) {
      this.remove(existing.id);
      return;
    }
    const bookmark: Bookmark = {
      id: newBookmarkEntityId(),
      folderId: null,
      surah,
      ayah,
      endSurah,
      endAyah,
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
        endSurah,
        endAyah,
        updatedAt: bookmark.updatedAt,
      },
      () => {
        // Durable list only — the overlay is in #pendingById already.
        if (this.#bookmarks.some((b) => bookmarkRangeKeyOf(b) === bookmarkRangeKeyOf(bookmark))) {
          return;
        }
        this.#bookmarks = [...this.#bookmarks, bookmark];
      },
      { bookmark },
    );
  }

  remove(bookmarkId: string): void {
    if (this.#captureAccount() === null) return;
    const durable = this.#bookmarks.find((b) => b.id === bookmarkId);
    const row = durable ?? this.#pendingById.get(bookmarkId);
    if (row === undefined) return;
    this.#enqueue(
      { kind: "bookmark.delete", id: bookmarkId, updatedAt: nowIso() },
      () => {
        this.#bookmarks = this.#bookmarks.filter((b) => b.id !== bookmarkId);
      },
      // Key-scoped (EC-40) on the full range key: deleting a range never
      // hides a same-verse single from the snapshot.
      { deletedVerseKey: bookmarkRangeKeyOf(row) },
    );
  }

  /** Undo for a just-removed row (BM-04): same entity, fresh timestamp — wins LWW over the pending delete and clears the tombstone (§9.3). */
  restore(bookmark: Bookmark): void {
    if (this.#captureAccount() === null) return;
    const restored: Bookmark = { ...bookmark, updatedAt: nowIso() };
    this.#enqueue(
      {
        kind: "bookmark.upsert",
        id: restored.id,
        folderId: restored.folderId,
        surah: restored.surah,
        ayah: restored.ayah,
        endSurah: restored.endSurah ?? null,
        endAyah: restored.endAyah ?? null,
        updatedAt: restored.updatedAt,
      },
      () => {
        if (this.#bookmarks.some((b) => b.id === restored.id)) return;
        this.#bookmarks = [...this.#bookmarks, restored];
      },
      { bookmark: restored },
    );
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
        endSurah: updated.endSurah ?? null,
        endAyah: updated.endAyah ?? null,
        updatedAt: updated.updatedAt,
      },
      () => {
        this.#bookmarks = this.#bookmarks.map((b) => (b.id === bookmarkId ? updated : b));
      },
      { bookmark: updated },
    );
  }

  createFolder(name: string, kind: BookmarkFolderKind = "generic"): BookmarkFolder | null {
    if (this.#captureAccount() === null || !isValidFolderName(name)) return null;
    const folder: BookmarkFolder = {
      id: newBookmarkEntityId(),
      name: name.trim(),
      kind,
      position: null,
      createdAt: nowIso(),
      updatedAt: nowIso(),
    };
    this.#enqueue(
      {
        kind: "folder.upsert",
        id: folder.id,
        name: folder.name,
        folderKind: folder.kind,
        position: null,
        updatedAt: folder.updatedAt,
      },
      () => {
        if (this.#folders.some((existing) => existing.id === folder.id)) return;
        this.#folders = [...this.#folders, folder];
      },
      { folder },
    );
    return folder;
  }

  renameFolder(id: string, name: string): void {
    if (!isValidFolderName(name)) return;
    this.updateFolder(id, { name: name.trim() });
  }

  /** Partial folder update (name / kind / position): the whole row re-upserts, so untouched fields ride along (§9.3 row-level LWW). */
  updateFolder(
    id: string,
    patch: { name?: string; kind?: BookmarkFolderKind; position?: number | null },
  ): void {
    if (this.#captureAccount() === null) return;
    if (patch.name !== undefined && !isValidFolderName(patch.name)) return;
    const existing = this.#folders.find((folder) => folder.id === id);
    if (!existing) return;
    // Explicit per-field assignment (not conditional spreads): each property is
    // always present, taking the patch value or falling back to the existing row.
    const updated: BookmarkFolder = {
      ...existing,
      name: patch.name !== undefined ? patch.name : existing.name,
      kind: patch.kind !== undefined ? patch.kind : existing.kind,
      position: patch.position !== undefined ? patch.position : existing.position,
      updatedAt: nowIso(),
    };
    this.#enqueue(
      {
        kind: "folder.upsert",
        id,
        name: updated.name,
        folderKind: updated.kind,
        position: updated.position,
        updatedAt: updated.updatedAt,
      },
      () => {
        this.#folders = this.#folders.map((folder) => (folder.id === id ? updated : folder));
      },
      { folder: updated },
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
        this.#mutationEntityIds.delete(mutation.id);
      }
    }
    const deletedVerses = new Set(this.#pendingDeletes.values());
    const newestPerVerse = new Map<string, Bookmark>();
    for (const bookmark of snapshot.bookmarks) {
      if (this.#pendingDeletes.has(bookmark.id)) continue;
      // Range-scoped key (7.4): a range and its start-verse single collapse
      // independently instead of overwriting each other.
      const key = bookmarkRangeKeyOf(bookmark);
      // EC-40: the delete is pending for the verse key, not just this id — a
      // rival row on the same key from a stale snapshot cannot re-mark it.
      if (deletedVerses.has(key)) continue;
      const prev = newestPerVerse.get(key);
      // EC-35: deterministic tie-break `(updatedAt, id)` — the collapse must
      // not depend on snapshot iteration order, so repeated syncs converge
      // identically on every device.
      if (
        prev === undefined ||
        bookmark.updatedAt > prev.updatedAt ||
        (bookmark.updatedAt === prev.updatedAt && bookmark.id > prev.id)
      ) {
        newestPerVerse.set(key, bookmark);
      }
    }
    for (const pending of this.#pendingById.values()) {
      newestPerVerse.set(bookmarkRangeKeyOf(pending), pending);
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
    this.#installDrainListener();
    // Load durable pending counts — including any retained pre-partition
    // entries (EC-48) — before the first auth edge renders a view.
    void this.#engine.hydrate?.();
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

  /**
   * Retained pre-partition entries (EC-48, §9.5): mutations still queued in
   * the legacy unowned `bookmarks` domain. Never discarded on hydrate, never
   * auto-attributed — the merge review surfaces them once for an explicit
   * apply-or-discard choice.
   */
  unownedPending(): number {
    return this.#engine.pendingFor(BOOKMARKS_DOMAIN);
  }

  /** Merge-review action: attribute the retained unowned mutations to `ownerId` (the user's explicit choice), then flush its partition. Resolves the moved count. */
  async applyLegacyUnowned(ownerId: number): Promise<number> {
    if (this.#suspended || this.#currentOwnerId() !== ownerId) return 0;
    const domain = bookmarksDomainFor(ownerId);
    const moved = await this.#engine.reassignDomain(BOOKMARKS_DOMAIN, domain);
    if (moved > 0 && !this.#suspended && this.#currentOwnerId() === ownerId) {
      await this.#engine.flush(domain);
    }
    return moved;
  }

  /** Merge-review action: discard the retained unowned mutations (explicit choice). */
  async discardLegacyUnowned(): Promise<void> {
    await this.#engine.clear(BOOKMARKS_DOMAIN);
  }

  /**
   * §9.6/EC-33: subscribe to other tabs' drain notices. App-lifetime wiring —
   * the app singleton never closes; tests construct isolated stores.
   */
  #installDrainListener(): void {
    if (this.#drainChannel !== null || !("BroadcastChannel" in globalThis)) return;
    const channel = new BroadcastChannel(SYNC_DRAIN_CHANNEL);
    channel.onmessage = (event: MessageEvent<unknown>): void => {
      const message = decodeDrainMessage(event.data);
      if (message === null) return;
      this.#onRemoteDrain(message);
    };
    this.#drainChannel = channel;
  }

  /**
   * Another tab drained these ids from the shared outbox: this tab's optimistic
   * overlays for them are stale (their mutations are gone and the server holds
   * the rows). Retire the overlays, refresh the durable pending counts, and
   * pull so the snapshot rows replace them without flicker. A partition that is
   * not the signed-in account's is ignored (EC-12/EC-17: never rendered, never
   * flushed for another owner).
   */
  #onRemoteDrain(message: SyncDrainMessage): void {
    const ownerId = this.#currentOwnerId();
    if (ownerId === null || this.#suspended) return;
    if (message.domain !== bookmarksDomainFor(ownerId)) return;
    for (const id of message.ids) {
      const entityId = this.#mutationEntityIds.get(id);
      this.#mutationEntityIds.delete(id);
      if (entityId === undefined) continue;
      this.#pendingById.delete(entityId);
      this.#pendingDeletes.delete(entityId);
      this.#pendingFoldersById.delete(entityId);
      this.#pendingFolderDeletes.delete(entityId);
    }
    void this.#engine.hydrate?.();
    void this.#engine.flush(message.domain).catch(() => undefined);
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
    if (this.#firstSyncRetryTimer !== null) {
      clearTimeout(this.#firstSyncRetryTimer);
      this.#firstSyncRetryTimer = null;
    }
    this.#firstSyncRetries = 0;
    this.#generation += 1;
    this.#ownerId = ownerId;
    this.#syncGeneration = null;
    this.#syncedAt = null;
    this.#folders = [];
    this.#bookmarks = [];
    this.#pendingById.clear();
    this.#pendingDeletes.clear();
    this.#pendingFoldersById.clear();
    this.#pendingFolderDeletes.clear();
    this.#mutationEntityIds.clear();
  }

  async #beginSync(account: AccountGeneration): Promise<void> {
    if (this.#syncGeneration === account.generation) return;
    this.#syncGeneration = account.generation;
    if (!this.#isCurrent(account)) return;
    // No silent legacy upload anymore (§9.4): local data reaches the account
    // only through the merge review's explicit choice (EC-08).
    const domain = createBookmarksDomain(
      this.#auth,
      this.#state,
      account.ownerId,
      () => this.#isCurrent(account),
      () => {
        if (!this.#isCurrent(account)) return;
        this.suspendAuth();
        // EC-15/16: cookie account changed mid-flight — hand the loss to the
        // auth-state seam (probe confirms; anonymous ⇒ terminate transition +
        // expiry banner). The account's outbox partition is left quarantined.
        this.#state.notifySessionLost?.();
      },
    );
    const adapter: SyncDomain<BookmarksMutation, BookmarksSnapshot> = {
      name: domain.name,
      sync: async (mutations, signal) => {
        try {
          return await domain.sync(mutations, signal);
        } catch (err) {
          // The engine skips paused rounds without a retry; if this was the
          // account's first pull, the store schedules its own backstop so the
          // view cannot sit never-synced until the next periodic trigger.
          if (err instanceof SyncPausedError) this.#scheduleFirstSyncRetry(account);
          throw err;
        }
      },
      applyServer: (state, drained) => {
        if (this.#isCurrent(account)) {
          // Per-domain truth: only a snapshot that landed on the live view
          // counts as this domain having synced.
          this.#syncedAt = Date.now();
          this.applyServer(state, drained);
        }
      },
    };
    this.#register(adapter);
    this.#engine.start();
    await this.#engine.flush(account.domain);
  }

  /**
   * Bounded backstop for a paused FIRST sync round (round-2): the engine drops
   * paused rounds silently, so the store wakes itself up — only while the
   * paused account is still the live view and no server state has landed.
   * Account-mismatch pauses suspend the view first, so they never retry here.
   */
  #scheduleFirstSyncRetry(account: AccountGeneration): void {
    if (this.#firstSyncRetryTimer !== null) return;
    if (this.#syncedAt !== null) return;
    if (!this.#isCurrent(account)) return;
    if (this.#firstSyncRetries >= FIRST_SYNC_RETRY_MAX) return;
    this.#firstSyncRetries += 1;
    this.#firstSyncRetryTimer = setTimeout(() => {
      this.#firstSyncRetryTimer = null;
      if (!this.#isCurrent(account)) return;
      void this.#engine.flush(account.domain).catch(() => undefined);
    }, FIRST_SYNC_RETRY_MS);
  }

  #enqueue(
    payload: BookmarksMutation,
    apply: () => void,
    overlay?: { bookmark?: Bookmark; folder?: BookmarkFolder; deletedVerseKey?: string },
  ): void {
    const account = this.#captureAccount();
    if (account === null) return;
    if (payload.kind === "bookmark.delete") {
      // "" never matches a verse key, so an unset key degrades to id-scoped only.
      this.#pendingDeletes.set(payload.id, overlay?.deletedVerseKey ?? "");
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
      (mutation) => {
        if (!this.#isCurrent(account)) return;
        this.#mutationEntityIds.set(mutation.id, payload.id);
        this.#enqueueError = null;
        apply();
      },
      // eslint-disable-next-line anti-slop/no-unknown-parameters -- rejection reason is any thrown value (opaque boundary); errorMessage() narrows it to a displayable string.
      (err: unknown) => {
        if (!this.#isCurrent(account)) return;
        // EC-27: a failed durable write is surfaced, never silently dropped.
        this.#enqueueError = errorMessage(err);
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
