import type { RegisteredSyncDomain, SyncMutation, SyncStatus } from "$lib/sync";
import type { VerseKey } from "$lib/data/quran";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { createBookmarksStore, type SyncEngineLike } from "../store.svelte";
import type { BookmarksMutation, BookmarksSnapshot } from "../schema";

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

const MARKER_KEY = "eq.bookmarks.legacy-migrated.7";

/** Let the async marker write (post-enqueue continuation) settle. */
async function flush(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

interface FakeEngine {
  readonly engine: SyncEngineLike;
  readonly enqueued: BookmarksMutation[];
  readonly flushed: Array<string | undefined>;
  readonly cleared: string[];
  readonly started: number;
  /** When true, the next enqueue() rejects (simulated durable-storage failure). */
  failNext: boolean;
  respondWithStatus(status: SyncStatus): void;
}

function fakeEngine(): FakeEngine {
  const enqueued: BookmarksMutation[] = [];
  const flushed: Array<string | undefined> = [];
  const cleared: string[] = [];
  const handle: FakeEngine & { started: number; status: SyncStatus } = {
    started: 0,
    failNext: false,
    status: { phase: "idle", pending: 0, lastSyncAt: null, lastError: null },
    enqueued,
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
        // SAFETY: test double — a minimal SyncMutation shape; the store never reads the returned mutation.
        return { id: `m${enqueued.length}`, seq: enqueued.length } as SyncMutation<P>;
      },
      async clear(domain?: string) {
        if (domain !== undefined) cleared.push(domain);
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
    { id: "f1", name: "Tafsir", createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
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
    // Payload is captured synchronously; the optimistic row lands only after
    // the durable enqueue resolves (durability-before-view contract).
    expect(rig.engine.enqueued).toHaveLength(1);
    expect(rig.store.isBookmarked(1, 1)).toBe(false);
    await flush();

    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.isMarkedKey("1:1")).toBe(true);
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
    expect(rig.engine.enqueued[0]!).toEqual({ kind: "bookmark.delete", id: "b1", updatedAt: expect.any(String) });
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
    expect(rig.engine.enqueued[1]).toMatchObject({ kind: "folder.upsert", id: folder.id, name: "New" });
  });

  it("deleteFolder detaches children and enqueues folder.delete", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.deleteFolder("f1");
    await flush();

    expect(rig.store.folders).toEqual([]);
    expect(rig.store.bookmarksIn(null)).toHaveLength(2);
    expect(rig.engine.enqueued).toEqual([{ kind: "folder.delete", id: "f1", updatedAt: expect.any(String) }]);
  });

  it("moveToFolder enqueues an upsert with the new folderId; unknown folder is a no-op", async () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.moveToFolder("b2", "f1");
    await flush();
    expect(rig.store.bookmarksIn("f1")).toHaveLength(2);
    expect(rig.engine.enqueued[0]).toMatchObject({ kind: "bookmark.upsert", id: "b2", folderId: "f1", surah: 112, ayah: 1 });

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
});

describe("BookmarksStore — auth transitions and migration", () => {
  it("hydrate with a session registers the domain, migrates legacy keys and flushes", async () => {
    const rig = makeRig(["2:255", "112:1"], true);
    rig.store.hydrate();
    await flush();

    expect(rig.registered).toHaveLength(1);
    expect(rig.registered[0]!.name).toBe("bookmarks");
    expect(rig.engine.started).toBe(1);
    expect(rig.engine.flushed).toEqual(["bookmarks"]);
    expect(rig.engine.enqueued).toEqual([
      expect.objectContaining({ kind: "bookmark.upsert", surah: 2, ayah: 255 }),
      expect.objectContaining({ kind: "bookmark.upsert", surah: 112, ayah: 1 }),
    ]);
    // Migration never deletes legacy data.
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

  it("writes the durable per-account marker only after the migration enqueues resolve", async () => {
    const rig = makeRig(["2:255"], true);
    rig.store.hydrate();
    expect(localStorage.getItem(MARKER_KEY)).toBeNull(); // not yet: enqueues unresolved

    await flush();
    expect(localStorage.getItem(MARKER_KEY)).toBe("1");
  });

  it("a fresh store instance (reload) over the same localStorage skips migration", async () => {
    const first = makeRig(["2:255", "112:1"], true);
    first.store.hydrate();
    await flush();
    expect(first.engine.enqueued).toHaveLength(2);

    const second = makeRig(["2:255", "112:1"], true);
    second.store.hydrate();
    await flush();

    expect(second.engine.enqueued).toEqual([]);
    expect(second.engine.flushed).toEqual(["bookmarks"]);
  });

  it("logout clears the queue durably, resets the view and the in-session guard; re-login skips the marked account", async () => {
    const rig = makeRig(["2:255"], true);
    rig.store.hydrate();
    await flush();
    expect(rig.engine.enqueued).toHaveLength(1);

    rig.store.applyServer(SNAP);
    rig.store.toggle(112, 1); // optimistic row + queued delete
    rig.setAuthed(false);
    rig.store.onAuthChanged(false);

    expect(rig.store.bookmarks).toEqual([]);
    expect(rig.store.folders).toEqual([]);
    expect(rig.engine.cleared).toEqual(["bookmarks"]);

    // Re-login as the same account: the durable marker gates re-migration.
    rig.reader.setKeys(["2:255", "3:26"]);
    rig.setAuthed(true);
    rig.store.onAuthChanged(true);
    await flush();

    expect(rig.engine.enqueued).toHaveLength(2); // only the pre-logout toggle
    expect(localStorage.getItem(MARKER_KEY)).toBe("1");
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

  it("migrates again for a different account id", async () => {
    const first = makeRig(["2:255"], true, 7);
    first.store.hydrate();
    await flush();
    expect(localStorage.getItem(MARKER_KEY)).toBe("1");

    const second = makeRig(["2:255"], true, 9);
    second.store.hydrate();
    await flush();

    expect(second.engine.enqueued).toHaveLength(1);
    expect(localStorage.getItem("eq.bookmarks.legacy-migrated.9")).toBe("1");
  });
});

describe("BookmarksStore — optimistic overlay", () => {
  function drainedOf(rig: Rig, payload: BookmarksMutation): SyncMutation<BookmarksMutation>[] {
    const index = rig.engine.enqueued.indexOf(payload);
    // SAFETY: test builds this from a just-enqueued payload; the id mirrors the fake engine's allocation.
    return [{ id: `m${index + 1}`, domain: "bookmarks", seq: index + 1, payload, queuedAt: 0 }];
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
    rig.store.applyServer(SNAP, drainedOf(rig, { kind: "folder.upsert", id: "unrelated", name: "x", updatedAt: "2026-01-01T00:00:00Z" }));

    expect(rig.store.isBookmarked(2, 255)).toBe(false);
    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.bookmarks).toHaveLength(2);

    // The toggle's own drain ack arrives with the fresh server truth.
    const fresh: BookmarksSnapshot = {
      folders: SNAP.folders,
      bookmarks: [
        { id: "b2", folderId: null, surah: 112, ayah: 1, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-02T00:00:00Z" },
        { id: "b4", folderId: null, surah: 1, ayah: 1, createdAt: "2026-01-05T00:00:00Z", updatedAt: "2026-01-05T00:00:00Z" },
      ],
    };
    rig.store.applyServer(fresh, [
      ...drainedOf(rig, removePayload),
      ...drainedOf(rig, addPayload),
    ]);

    // Overlay entries for the drained entities are retired; server rows win.
    expect(rig.store.bookmarks.map((b) => b.id)).toEqual(["b2", "b4"]);
    expect(rig.store.isBookmarked(2, 255)).toBe(false);
  });

  it("logout drops the overlay along with the view", async () => {
    const rig = makeRig([], true);
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
});

describe("BookmarksStore — view grouping", () => {
  it("files orphaned folderId rows under the unfiled group instead of hiding them", () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);
    rig.store.applyServer({
      folders: SNAP.folders,
      bookmarks: [
        ...SNAP.bookmarks,
        { id: "orphan", folderId: "ghost", surah: 3, ayah: 26, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
      ],
    });

    const groups = rig.store.bookmarksByFolder();
    expect(groups.get(null)!.map((b) => b.id)).toEqual(["b2", "orphan"]);
    expect(groups.get("f1")!.map((b) => b.id)).toEqual(["b1"]);
    // Exact-match helper stays untouched for existing consumers.
    expect(rig.store.bookmarksIn("ghost")).toHaveLength(1);
  });
});
