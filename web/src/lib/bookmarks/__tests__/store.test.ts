import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { VerseKey } from "#lib/data/quran.js";
import {
  SyncPausedError,
  type RegisteredSyncDomain,
  type SyncMutation,
  type SyncStatus,
} from "#lib/sync/index.js";

import type { BookmarksMutation, BookmarksSnapshot } from "../schema";
import { createBookmarksStore, type SyncEngineLike } from "../store.svelte";
import { SYNC_PAGE_NONCE } from "#lib/sync/engine.svelte.js";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

const MARKER_KEY = "eq.bookmarks.legacy-migrated.7";

/** Let the async marker write (post-enqueue continuation) settle. */
async function flush(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

interface FakeEngine {
  readonly engine: SyncEngineLike;
  readonly enqueued: BookmarksMutation[];
  /** Outbox mutation ids in enqueue order — what the §9.6 drain broadcast carries. */
  readonly mutationIds: string[];
  readonly flushed: Array<string | undefined>;
  readonly cleared: string[];
  readonly started: number;
  /** When true, the next enqueue() rejects (simulated durable-storage failure). */
  failNext: boolean;
  respondWithStatus(status: SyncStatus): void;
}

function fakeEngine(): FakeEngine {
  const enqueued: BookmarksMutation[] = [];
  const mutationIds: string[] = [];
  const flushed: Array<string | undefined> = [];
  const cleared: string[] = [];
  const handle: FakeEngine & { started: number; status: SyncStatus } = {
    started: 0,
    failNext: false,
    status: { phase: "idle", pending: 0, lastSyncAt: null, lastError: null },
    enqueued,
    mutationIds,
    flushed,
    cleared,
    respondWithStatus(status: SyncStatus) {
      handle.status = status;
    },
    engine: {
      async enqueue<P>(_domain: string, payload: P) {
        if (handle.failNext) {
          handle.failNext = false;
          throw new Error("idb full");
        }
        // SAFETY: test double — the only producer is BookmarksStore.#enqueue, whose payloads are BookmarksMutation by construction.
        enqueued.push(payload as BookmarksMutation);
        const id = `m${enqueued.length}`;
        mutationIds.push(id);
        // SAFETY: test double — a minimal SyncMutation shape; the store never reads the returned mutation.
        return { id, seq: enqueued.length } as SyncMutation<P>;
      },
      async clear(domain?: string) {
        if (domain !== undefined) cleared.push(domain);
      },
      async reassignDomain() {
        return 0;
      },
      pendingFor() {
        return handle.status.pending;
      },
      async flush(domain?: string) {
        flushed.push(domain);
      },
      start() {
        handle.started += 1;
        return () => undefined;
      },
      get pending() {
        return handle.status.pending;
      },
      get status() {
        return handle.status;
      },
    },
  };
  return handle;
}

interface FakeReader {
  readonly reader: {
    isBookmarked(key: VerseKey): boolean;
    toggleBookmark(key: VerseKey): void;
    readonly bookmarkedKeys: readonly VerseKey[];
  };
  readonly toggled: VerseKey[];
  setKeys(keys: VerseKey[]): void;
}

function fakeReader(initialKeys: VerseKey[] = []): FakeReader {
  const toggled: VerseKey[] = [];
  const handle: FakeReader & { keys: VerseKey[] } = {
    keys: [...initialKeys],
    toggled,
    setKeys(keys: VerseKey[]) {
      handle.keys = keys;
    },
    reader: {
      isBookmarked: (key) => handle.keys.includes(key),
      toggleBookmark: (key) => {
        toggled.push(key);
        const index = handle.keys.indexOf(key);
        if (index >= 0) handle.keys.splice(index, 1);
        else handle.keys.push(key);
      },
      get bookmarkedKeys() {
        return handle.keys;
      },
    },
  };
  return handle;
}

interface Rig {
  readonly store: ReturnType<typeof createBookmarksStore>;
  readonly engine: FakeEngine;
  readonly reader: FakeReader;
  readonly registered: RegisteredSyncDomain[];
  setAuthed(value: boolean, userId?: number): void;
}

function makeRig(initialLegacyKeys: VerseKey[] = [], authed = false, userId = 7): Rig {
  const engine = fakeEngine();
  const reader = fakeReader(initialLegacyKeys);
  const registered: RegisteredSyncDomain[] = [];
  const state = { authenticated: authed, user: authed ? { id: userId } : null };
  const store = createBookmarksStore({
    engine: engine.engine,
    auth: {
      async unsafeRequest() {
        throw new Error("store tests never hit the wire");
      },
    },
    reader: reader.reader,
    state,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  return {
    store,
    engine,
    reader,
    registered,
    setAuthed(value: boolean, nextUserId = userId) {
      state.authenticated = value;
      state.user = value ? { id: nextUserId } : null;
    },
  };
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const SNAP: BookmarksSnapshot = {
  folders: [
    {
      id: "f1",
      name: "Tafsir",
      kind: "generic",
      position: null,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    },
  ],
  bookmarks: [
    {
      id: "b1",
      folderId: "f1",
      surah: 2,
      ayah: 255,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-01T00:00:00Z",
    },
    {
      id: "b2",
      folderId: null,
      surah: 112,
      ayah: 1,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-02T00:00:00Z",
    },
  ],
};

describe("BookmarksStore — anonymous view", () => {
  it("toggle delegates to the legacy reader and never enqueues", () => {
    const rig = makeRig();
    rig.store.toggle(2, 255);
    expect(rig.reader.toggled).toEqual(["2:255"]);
    expect(rig.engine.enqueued).toEqual([]);
  });

  it("toggleVerse delegates for malformed keys, isMarkedKey reads legacy state", () => {
    const rig = makeRig(["1:1"]);
    rig.store.toggleVerse("bogus");
    expect(rig.reader.toggled).toEqual(["bogus"]);
    expect(rig.store.isMarkedKey("1:1")).toBe(true);
    expect(rig.store.isMarkedKey("9:9")).toBe(false);
  });

  it("folder ops are auth-only no-ops", () => {
    const rig = makeRig();
    expect(rig.store.createFolder("X")).toBeNull();
    rig.store.renameFolder("f1", "Y");
    rig.store.deleteFolder("f1");
    rig.store.moveToFolder("b1", null);
    expect(rig.engine.enqueued).toEqual([]);
    expect(rig.store.folders).toEqual([]);
  });
});

describe("BookmarksStore — authed toggles", () => {
  it("toggle enqueues a bookmark.upsert, then adds the row once the write is durable", async () => {
    const rig = makeRig([], true);
    rig.store.toggle(1, 1);
    // Payload is captured synchronously; the durable row lands only after the
    // enqueue resolves (durability-before-view contract), but the pending
    // overlay already reads the verse as marked.
    expect(rig.engine.enqueued).toHaveLength(1);
    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.bookmarks).toHaveLength(0);
    await flush();

    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.isMarkedKey("1:1")).toBe(true);
    expect(rig.store.bookmarks).toHaveLength(1);
    const payload = rig.engine.enqueued[0]!;
    expect(payload.kind).toBe("bookmark.upsert");
    if (payload.kind !== "bookmark.upsert") return;
    expect(payload.surah).toBe(1);
    expect(payload.ayah).toBe(1);
    expect(payload.folderId).toBeNull();
    expect(payload.updatedAt).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
    const added = rig.store.bookmarks[0]!;
    expect(added.id).toBe(payload.id);
    expect(rig.store.bookmarksIn(null)).toHaveLength(1);
  });

  it("toggle removes an existing bookmark and enqueues a bookmark.delete", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);
    rig.store.toggle(2, 255);
    await flush();

    expect(rig.store.isBookmarked(2, 255)).toBe(false);
    expect(rig.store.bookmarks).toHaveLength(1);
    expect(rig.engine.enqueued).toHaveLength(1);
    expect(rig.engine.enqueued[0]!).toEqual({
      kind: "bookmark.delete",
      id: "b1",
      updatedAt: expect.any(String),
    });
  });

  it("a rejected enqueue applies no optimistic view and leaves no overlay behind", async () => {
    const rig = makeRig([], true);
    rig.engine.failNext = true;
    rig.store.toggle(1, 1);
    await flush();

    // No durable write happened: the view never shows the toggle...
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    expect(rig.engine.enqueued).toEqual([]);
    // ...and a later snapshot lands unfiltered (no orphaned overlay entries).
    rig.store.applyServer(SNAP);
    expect(rig.store.bookmarks).toHaveLength(2);
  });

  it("a rejected enqueue surfaces the failure instead of silently dropping it (EC-27)", async () => {
    const rig = makeRig([], true);
    rig.engine.failNext = true;
    rig.store.toggle(1, 1);
    await flush();

    expect(rig.store.enqueueError).toBe("idb full");

    // The next durable write clears the surfaced error.
    rig.store.toggle(1, 1);
    await flush();
    expect(rig.store.enqueueError).toBeNull();
    expect(rig.store.isBookmarked(1, 1)).toBe(true);
  });

  it("a double-fire before the optimistic apply cancels the same entity instead of minting a second upsert", async () => {
    const rig = makeRig([], true);
    rig.store.toggle(1, 1);
    const entityId = rig.engine.enqueued[0]!.id;
    // Second fire lands before the first enqueue's optimistic apply: the
    // pending overlay must read as "on" for the verse.
    rig.store.toggle(1, 1);
    await flush();

    expect(rig.engine.enqueued.map((m) => m.kind)).toEqual(["bookmark.upsert", "bookmark.delete"]);
    expect(rig.engine.enqueued[1]).toMatchObject({ kind: "bookmark.delete", id: entityId });
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
  });

  it("reads follow the pending overlay during the durability window", async () => {
    const rig = makeRig([], true);
    rig.store.toggle(1, 1);
    // Upsert enqueued but not yet durable: the verse already reads as marked...
    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.isMarkedKey("1:1")).toBe(true);
    // ...so the second fire enqueues the compensating delete, not a new upsert.
    rig.store.toggle(1, 1);
    expect(rig.engine.enqueued.map((m) => m.kind)).toEqual(["bookmark.upsert", "bookmark.delete"]);
    // The delete retired the overlay row: unmarked again during that window.
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    expect(rig.store.isMarkedKey("1:1")).toBe(false);

    await flush();
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    expect(rig.store.bookmarks).toHaveLength(0);
  });

  it("restore() re-upserts the removed row with a fresher timestamp (BM-04 undo)", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);
    const before = rig.store.bookmarks.find((b) => b.id === "b1");
    if (!before) throw new Error("b1 missing");

    rig.store.remove("b1");
    await flush();
    expect(rig.store.isBookmarked(2, 255)).toBe(false);

    rig.store.restore({ ...before, updatedAt: before.updatedAt });
    await flush();
    expect(rig.store.isBookmarked(2, 255)).toBe(true);
    const undo = rig.engine.enqueued[1]!;
    expect(undo).toMatchObject({ kind: "bookmark.upsert", id: "b1", surah: 2, ayah: 255 });
    expect(undo.updatedAt > before.updatedAt).toBe(true);
  });

  it("remove() on a durable-but-unapplied pending row enqueues the compensating delete and retires the overlay", async () => {
    const rig = makeRig([], true);
    rig.store.toggle(1, 1);
    const entityId = rig.engine.enqueued[0]!.id; // in #pendingById, not yet in #bookmarks
    rig.store.remove(entityId);
    await flush();

    expect(rig.engine.enqueued[1]).toMatchObject({ kind: "bookmark.delete", id: entityId });
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    // Overlay retired as a delete: a stale snapshot carrying the row cannot
    // resurrect it.
    rig.store.applyServer({
      folders: [],
      bookmarks: [
        {
          id: entityId,
          folderId: null,
          surah: 1,
          ayah: 1,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ],
    });
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
  });
});

describe("BookmarksStore — folders and moves", () => {
  it("createFolder enqueues folder.upsert and stores the trimmed name", async () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("  Tafsir  ");
    await flush();

    expect(folder?.name).toBe("Tafsir");
    expect(rig.store.folders).toHaveLength(1);
    expect(rig.engine.enqueued[0]).toMatchObject({ kind: "folder.upsert", name: "Tafsir" });
  });

  it("createFolder carries the study-set kind; updateFolder re-labels kind and position LWW", async () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("Daily", "daily");
    await flush();
    expect(folder?.kind).toBe("daily");
    expect(rig.engine.enqueued[0]).toMatchObject({
      kind: "folder.upsert",
      folderKind: "daily",
      position: null,
    });

    rig.store.updateFolder(folder!.id, { kind: "learning", position: 2 });
    await flush();
    expect(rig.store.folderById(folder!.id)).toMatchObject({ kind: "learning", position: 2 });
    // The whole row re-upserts: untouched fields ride along, never reset.
    expect(rig.engine.enqueued[1]).toMatchObject({
      kind: "folder.upsert",
      name: "Daily",
      folderKind: "learning",
      position: 2,
    });
  });

  it("rejects empty/over-long folder names", () => {
    const rig = makeRig([], true);
    expect(rig.store.createFolder("   ")).toBeNull();
    expect(rig.store.createFolder("x".repeat(101))).toBeNull();
    expect(rig.engine.enqueued).toEqual([]);
  });

  it("renameFolder enqueues an upsert with the new name", async () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("Old");
    if (!folder) throw new Error("folder missing");
    await flush();
    rig.store.renameFolder(folder.id, "New");
    await flush();

    expect(rig.store.folderById(folder.id)?.name).toBe("New");
    expect(rig.engine.enqueued[1]).toMatchObject({
      kind: "folder.upsert",
      id: folder.id,
      name: "New",
    });
  });

  it("deleteFolder detaches children and enqueues folder.delete", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.deleteFolder("f1");
    await flush();

    expect(rig.store.folders).toEqual([]);
    expect(rig.store.bookmarksIn(null)).toHaveLength(2);
    expect(rig.engine.enqueued).toEqual([
      { kind: "folder.delete", id: "f1", updatedAt: expect.any(String) },
    ]);
  });

  it("moveToFolder enqueues an upsert with the new folderId; unknown folder is a no-op", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.moveToFolder("b2", "f1");
    await flush();
    expect(rig.store.bookmarksIn("f1")).toHaveLength(2);
    expect(rig.engine.enqueued[0]).toMatchObject({
      kind: "bookmark.upsert",
      id: "b2",
      folderId: "f1",
      surah: 112,
      ayah: 1,
    });

    rig.store.moveToFolder("b2", "missing");
    expect(rig.engine.enqueued).toHaveLength(1);

    rig.store.moveToFolder("b2", "f1");
    expect(rig.engine.enqueued).toHaveLength(1); // same folder: no-op
  });
});

describe("BookmarksStore — server snapshots", () => {
  it("applyServer replaces state and collapses duplicate verses to the newest", () => {
    const rig = makeRig([], true);
    rig.store.applyServer({
      ...SNAP,
      bookmarks: [
        ...SNAP.bookmarks,
        {
          id: "b3",
          folderId: null,
          surah: 2,
          ayah: 255,
          createdAt: "2026-01-03T00:00:00Z",
          updatedAt: "2026-01-03T00:00:00Z",
        },
      ],
    });

    expect(rig.store.bookmarks).toHaveLength(2);
    expect(rig.store.isBookmarked(2, 255)).toBe(true);
    expect(rig.store.bookmarks.find((b) => b.surah === 2 && b.ayah === 255)?.id).toBe("b3");
    expect(rig.store.folderById("f1")?.name).toBe("Tafsir");
  });

  it("a pending delete matches by verse key: a stale rival row cannot re-mark the verse (EC-40)", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP); // b1 lives on 2:255
    rig.store.remove("b1"); // delete in flight for verse 2:255
    await flush();

    // Stale snapshot carries a different-id rival row for the same verse.
    const rival = {
      id: "rival",
      folderId: null,
      surah: 2,
      ayah: 255,
      createdAt: "2026-01-04T00:00:00Z",
      updatedAt: "2026-01-04T00:00:00Z",
    };
    rig.store.applyServer({ folders: [], bookmarks: [SNAP.bookmarks[1]!, rival] });

    expect(rig.store.isBookmarked(2, 255)).toBe(false);
    expect(rig.store.isBookmarked(112, 1)).toBe(true);
  });

  it("collapses equal-timestamp rows deterministically by id, regardless of snapshot order (EC-35)", () => {
    const rig = makeRig([], true);
    const tieA = {
      id: "aaa",
      folderId: null,
      surah: 2,
      ayah: 255,
      createdAt: "2026-01-01T00:00:00Z",
      updatedAt: "2026-01-05T00:00:00Z",
    };
    const tieZ = { ...tieA, id: "zzz" };

    rig.store.applyServer({ folders: [], bookmarks: [tieZ, tieA] });
    expect(rig.store.bookmarks.find((b) => b.surah === 2 && b.ayah === 255)?.id).toBe("zzz");

    rig.store.applyServer({ folders: [], bookmarks: [tieA, tieZ] });
    expect(rig.store.bookmarks.find((b) => b.surah === 2 && b.ayah === 255)?.id).toBe("zzz");

    // A strictly newer row still beats the greater id.
    const newer = { ...tieA, updatedAt: "2026-01-06T00:00:00Z" };
    rig.store.applyServer({ folders: [], bookmarks: [newer, tieZ] });
    expect(rig.store.bookmarks.find((b) => b.surah === 2 && b.ayah === 255)?.id).toBe("aaa");
  });
});

describe("BookmarksStore — auth transitions and migration", () => {
  it("hydrate with a session registers the domain and flushes without migrating legacy keys (EC-08)", async () => {
    const rig = makeRig(["2:255", "112:1"], true);
    rig.store.hydrate();
    await flush();

    expect(rig.registered).toHaveLength(1);
    expect(rig.registered[0]!.name).toBe("bookmarks.user:7");
    expect(rig.engine.started).toBe(1);
    expect(rig.engine.flushed).toEqual(["bookmarks.user:7"]);
    // §9.4: the merge review owns the legacy upload — nothing is enqueued
    // before an explicit choice, and no marker is written by the store.
    expect(rig.engine.enqueued).toEqual([]);
    expect(localStorage.getItem(MARKER_KEY)).toBeNull();
    // Device data stays intact either way.
    expect(rig.reader.reader.bookmarkedKeys).toEqual(["2:255", "112:1"]);
  });

  it("hydrate is idempotent and a no-op while anonymous", async () => {
    const rig = makeRig(["1:1"], false);
    rig.store.hydrate();
    rig.store.hydrate();
    expect(rig.registered).toEqual([]);
    expect(rig.engine.enqueued).toEqual([]);
  });

  it("the registered adapter routes applyServer into store state", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate();
    await flush();
    rig.registered[0]!.applyServer(SNAP);
    expect(rig.store.bookmarks).toHaveLength(2);
  });

  it("a fresh store instance (reload) over the same localStorage never migrates silently", async () => {
    const first = makeRig(["2:255", "112:1"], true);
    first.store.hydrate();
    await flush();
    expect(first.engine.enqueued).toHaveLength(0);

    const second = makeRig(["2:255", "112:1"], true);
    second.store.hydrate();
    await flush();

    expect(second.engine.enqueued).toEqual([]);
    expect(second.engine.flushed).toEqual(["bookmarks.user:7"]);
    expect(localStorage.getItem(MARKER_KEY)).toBeNull();
  });

  it("cold-load unknown status retains the unowned queue for review (EC-48)", async () => {
    const rig = makeRig(["2:255"], false);
    rig.store.hydrate(); // anon boot: no-op
    rig.store.onAuthChanged(false);

    // Pre-partition entries are never discarded on hydrate or logout — only an
    // explicit merge-review choice removes them (EC-48).
    expect(rig.engine.cleared).toEqual([]);
    expect(rig.engine.enqueued).toEqual([]);

    // The probe resolves authed afterwards: sync starts normally.
    rig.setAuthed(true);
    rig.store.onAuthChanged(true);
    await flush();

    expect(rig.engine.flushed).toEqual(["bookmarks.user:7"]);
    expect(rig.engine.enqueued).toHaveLength(0); // legacy keys migrate only via the merge review

    rig.setAuthed(false);
    rig.store.onAuthChanged(false);
    expect(rig.engine.cleared).toEqual([]);
  });

  it("logout preserves the owner partition, resets the view; re-login never migrates silently", async () => {
    const rig = makeRig(["2:255"], true);
    rig.store.hydrate();
    await flush();
    expect(rig.engine.enqueued).toHaveLength(0);

    rig.store.applyServer(SNAP);
    rig.store.toggle(112, 1); // optimistic row + queued delete
    rig.setAuthed(false);
    rig.store.onAuthChanged(false);

    expect(rig.store.bookmarks).toEqual([]);
    expect(rig.store.folders).toEqual([]);
    // Logout wipes the view, never any queue partition (EC-14/EC-48).
    expect(rig.engine.cleared).toEqual([]);

    // Re-login as the same account: the legacy upload stays the merge review's
    // explicit choice — nothing new is enqueued, no marker appears.
    rig.reader.setKeys(["2:255", "3:26"]);
    rig.setAuthed(true);
    rig.store.onAuthChanged(true);
    await flush();

    expect(rig.engine.enqueued).toHaveLength(1); // only the pre-logout toggle
    expect(localStorage.getItem(MARKER_KEY)).toBeNull();
  });

  it("a snapshot from a round mid-flight at logout never repopulates the view", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate();
    await flush();
    rig.store.applyServer(SNAP);
    expect(rig.store.bookmarks).toHaveLength(2);

    rig.setAuthed(false);
    rig.store.onAuthChanged(false);
    // The in-flight round completes after the logout reset.
    rig.store.applyServer(SNAP);

    expect(rig.store.bookmarks).toEqual([]);
    expect(rig.store.folders).toEqual([]);
  });
});

describe("BookmarksStore — optimistic overlay", () => {
  function drainedOf(rig: Rig, payload: BookmarksMutation): SyncMutation<BookmarksMutation>[] {
    const index = rig.engine.enqueued.indexOf(payload);
    // SAFETY: test builds this from a just-enqueued payload; the id mirrors the fake engine's allocation.
    return [
      { id: `m${index + 1}`, domain: "bookmarks.user:7", seq: index + 1, payload, queuedAt: 0 },
    ];
  }

  it("a stale snapshot cannot flicker away in-flight optimistic edits", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    // Mid-flight edits: remove b1 (2:255) and add 1:1 optimistically.
    rig.store.toggle(2, 255);
    rig.store.toggle(1, 1);
    const removePayload = rig.engine.enqueued[0]!;
    const addPayload = rig.engine.enqueued[1]!;

    // An older round acks with a stale snapshot that still contains b1 and
    // knows nothing of the 1:1 add: both edits must survive.
    rig.store.applyServer(
      SNAP,
      drainedOf(rig, {
        kind: "folder.upsert",
        id: "unrelated",
        name: "x",
        updatedAt: "2026-01-01T00:00:00Z",
      }),
    );

    expect(rig.store.isBookmarked(2, 255)).toBe(false);
    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.bookmarks).toHaveLength(2);

    // The toggle's own drain ack arrives with the fresh server truth.
    const fresh: BookmarksSnapshot = {
      folders: SNAP.folders,
      bookmarks: [
        {
          id: "b2",
          folderId: null,
          surah: 112,
          ayah: 1,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-02T00:00:00Z",
        },
        {
          id: "b4",
          folderId: null,
          surah: 1,
          ayah: 1,
          createdAt: "2026-01-05T00:00:00Z",
          updatedAt: "2026-01-05T00:00:00Z",
        },
      ],
    };
    rig.store.applyServer(fresh, [...drainedOf(rig, removePayload), ...drainedOf(rig, addPayload)]);

    // Overlay entries for the drained entities are retired; server rows win.
    expect(rig.store.bookmarks.map((b) => b.id)).toEqual(["b2", "b4"]);
    expect(rig.store.isBookmarked(2, 255)).toBe(false);
  });

  it("logout drops the overlay along with the view", async () => {
    const rig = makeRig([], true);
    // Establish the true authed edge first (production always has one — the
    // probe resolving or a login) so the later false edge is a real logout.
    rig.store.hydrate();
    await flush();
    rig.store.applyServer(SNAP);
    rig.store.toggle(1, 1);

    rig.setAuthed(false);
    rig.store.onAuthChanged(false);

    // Re-login + a fresh snapshot: no zombie optimistic rows reappear.
    rig.setAuthed(true);
    rig.store.onAuthChanged(true);
    rig.store.applyServer(SNAP);

    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    expect(rig.store.bookmarks).toHaveLength(2);
  });

  it("an in-flight folder create/rename survives a stale pull-only snapshot", async () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("Tafsir");
    await flush();
    rig.store.renameFolder(folder!.id, "Tafsir 2");
    await flush();
    expect(rig.store.folderById(folder!.id)?.name).toBe("Tafsir 2");

    // A pull-only round (periodic/visibility) whose snapshot predates both
    // edits must not erase the folder or flicker the old name back.
    rig.store.applyServer({ folders: [], bookmarks: [] });
    expect(rig.store.folders.map((f) => f.name)).toEqual(["Tafsir 2"]);

    // The drain ack retires the overlay: server truth lands unfiltered.
    const acked = rig.engine.enqueued;
    rig.store.applyServer(
      { folders: [{ ...folder!, name: "Tafsir 2" }], bookmarks: [] },
      acked.map((payload, index) => ({
        id: `m${index + 1}`,
        domain: "bookmarks.user:7",
        seq: index + 1,
        payload,
        queuedAt: 0,
      })),
    );
    expect(rig.store.folders.map((f) => f.name)).toEqual(["Tafsir 2"]);
    rig.store.applyServer({ folders: [], bookmarks: [] });
    expect(rig.store.folders).toEqual([]);
  });

  it("an in-flight folder delete is not resurrected by a stale snapshot", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP); // f1 exists server-side

    rig.store.deleteFolder("f1");
    await flush();

    // Stale snapshot still lists f1: the pending-delete overlay filters it.
    rig.store.applyServer(SNAP);
    expect(rig.store.folders).toEqual([]);
  });
});

describe("BookmarksStore — view grouping", () => {
  it("files orphaned folderId rows under the unfiled group instead of hiding them", () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);
    rig.store.applyServer({
      folders: SNAP.folders,
      bookmarks: [
        ...SNAP.bookmarks,
        {
          id: "orphan",
          folderId: "ghost",
          surah: 3,
          ayah: 26,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ],
    });

    const groups = rig.store.bookmarksByFolder();
    expect(groups.get(null)!.map((b) => b.id)).toEqual(["b2", "orphan"]);
    expect(groups.get("f1")!.map((b) => b.id)).toEqual(["b1"]);
    // Exact-match helper stays untouched for existing consumers.
    expect(rig.store.bookmarksIn("ghost")).toHaveLength(1);
  });
});

describe("BookmarksStore — session loss on cookie-account mismatch (EC-15/16)", () => {
  it("a 403 account-changed rejection suspends the view and hands the loss to the auth seam", async () => {
    const engine = fakeEngine();
    const registered: RegisteredSyncDomain[] = [];
    const notifySessionLost = vi.fn();
    const store = createBookmarksStore({
      engine: engine.engine,
      auth: {
        async unsafeRequest() {
          return {
            ok: false,
            status: 403,
            data: null,
            error: { message: "Bookmark sync account changed" },
            rotated: false,
          };
        },
      },
      reader: fakeReader().reader,
      state: { authenticated: true, user: { id: 7 }, notifySessionLost },
      register: (domain) => registered.push(domain),
      unregister: () => undefined,
    });
    store.hydrate();
    await flush();
    store.applyServer(SNAP);
    expect(store.bookmarks).toHaveLength(2);

    await expect(registered[0]!.sync([], undefined)).rejects.toBeInstanceOf(SyncPausedError);
    expect(notifySessionLost).toHaveBeenCalledTimes(1);
    // Suspend hides the account view; the outbox partition is left in place.
    expect(store.authed).toBe(false);
    expect(store.bookmarks).toHaveLength(0);
    expect(engine.cleared).toEqual([]);
  });
});

describe("BookmarksStore — cross-tab drain retire (EC-33, §9.6)", () => {
  class MockSyncChannel {
    static instances: MockSyncChannel[] = [];
    onmessage: ((event: { data: unknown }) => void) | null = null;

    constructor() {
      MockSyncChannel.instances.push(this);
    }
    // eslint-disable-next-line anti-slop/no-unknown-parameters -- mirrors the DOM BroadcastChannel.postMessage contract; malformed payloads below deliberately feed the decoder's rejection paths.
    postMessage(data: unknown): void {
      // BroadcastChannel semantics: every *other* instance hears the message.
      for (const other of MockSyncChannel.instances) {
        if (other === this || other.onmessage === null) continue;
        other.onmessage({ data });
      }
    }
    close(): void {}
  }

  beforeEach(() => {
    MockSyncChannel.instances = [];
    vi.stubGlobal("BroadcastChannel", MockSyncChannel);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("retires overlays another tab drained and pulls that domain", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate(); // installs the drain listener
    rig.store.applyServer(SNAP);
    rig.store.toggle(3, 9);
    await flush();
    expect(rig.store.isBookmarked(3, 9)).toBe(true);

    const flushCalls = rig.engine.flushed.length;
    const drainedMutationId = rig.engine.mutationIds[0]!;
    const otherTab = new MockSyncChannel();
    otherTab.postMessage({ domain: "bookmarks.user:7", ids: [drainedMutationId] });

    // The drained partition gets a pull so the server rows replace the overlays.
    expect(rig.engine.flushed.length).toBe(flushCalls + 1);
    expect(rig.engine.flushed.at(-1)).toBe("bookmarks.user:7");
    // The retired overlay no longer survives a snapshot apply: the verse's only
    // local trace was the optimistic row the other tab already drained.
    rig.store.applyServer(SNAP);
    expect(rig.store.isBookmarked(3, 9)).toBe(false);
    expect(rig.store.bookmarks).toHaveLength(2);
  });

  it("ignores this page's own drain notices — the store retires those overlays itself in applyServer", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate();
    rig.store.applyServer(SNAP);
    rig.store.toggle(3, 9);
    await flush();

    const flushCalls = rig.engine.flushed.length;
    const drainedMutationId = rig.engine.mutationIds[0]!;
    const echo = new MockSyncChannel();
    echo.postMessage({
      domain: "bookmarks.user:7",
      ids: [drainedMutationId],
      page: SYNC_PAGE_NONCE,
    });

    expect(rig.engine.flushed.length).toBe(flushCalls); // no self-echo pull
    rig.store.applyServer(SNAP);
    expect(rig.store.isBookmarked(3, 9)).toBe(true); // same-page retire is applyServer's job
  });

  it("ignores drain notices for another account's partition (EC-12/EC-17)", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate();
    rig.store.applyServer(SNAP);
    rig.store.toggle(3, 9);
    await flush();

    const flushCalls = rig.engine.flushed.length;
    const drainedMutationId = rig.engine.mutationIds[0]!;
    const otherTab = new MockSyncChannel();
    otherTab.postMessage({ domain: "bookmarks.user:8", ids: [drainedMutationId] });

    expect(rig.engine.flushed.length).toBe(flushCalls); // no pull for a foreign partition
    rig.store.applyServer(SNAP);
    expect(rig.store.isBookmarked(3, 9)).toBe(true); // this tab's overlay stands
  });

  it("ignores malformed payloads", async () => {
    const rig = makeRig([], true);
    rig.store.hydrate();
    const flushCalls = rig.engine.flushed.length;
    const otherTab = new MockSyncChannel();
    otherTab.postMessage({ domain: "bookmarks.user:7", ids: [42] });
    otherTab.postMessage("noise");

    expect(rig.engine.flushed.length).toBe(flushCalls);
  });
});
