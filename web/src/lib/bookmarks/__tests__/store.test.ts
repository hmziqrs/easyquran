import type { RegisteredSyncDomain, SyncMutation, SyncStatus } from "$lib/sync";
import type { VerseKey } from "$lib/data/quran";
import { describe, expect, it, vi } from "vite-plus/test";

import { createBookmarksStore, type SyncEngineLike } from "../store.svelte";
import type { BookmarksMutation, BookmarksSnapshot } from "../schema";

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$env/dynamic/public", () => ({ env: { PUBLIC_API_BASE_URL: "https://eq.test/api" } }));

interface FakeEngine {
  readonly engine: SyncEngineLike;
  readonly enqueued: BookmarksMutation[];
  readonly flushed: Array<string | undefined>;
  readonly started: number;
  respondWithStatus(status: SyncStatus): void;
}

function fakeEngine(): FakeEngine {
  const enqueued: BookmarksMutation[] = [];
  const flushed: Array<string | undefined> = [];
  const handle: FakeEngine & { started: number; status: SyncStatus } = {
    started: 0,
    status: { phase: "idle", pending: 0, lastSyncAt: null, lastError: null },
    enqueued,
    flushed,
    respondWithStatus(status: SyncStatus) {
      handle.status = status;
    },
    engine: {
      async enqueue<P>(_domain: string, payload: P) {
        // SAFETY: test double — the only producer is BookmarksStore.#enqueue, whose payloads are BookmarksMutation by construction.
        enqueued.push(payload as BookmarksMutation);
        // SAFETY: test double — a minimal SyncMutation shape; the store never reads the returned mutation.
        return { id: `m${enqueued.length}`, seq: enqueued.length } as SyncMutation<P>;
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
  setAuthed(value: boolean): void;
}

function makeRig(initialLegacyKeys: VerseKey[] = [], authed = false): Rig {
  const engine = fakeEngine();
  const reader = fakeReader(initialLegacyKeys);
  const registered: RegisteredSyncDomain[] = [];
  const state = { authenticated: authed };
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
    setAuthed(value: boolean) {
      state.authenticated = value;
    },
  };
}

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
  it("toggle adds optimistically and enqueues a bookmark.upsert", () => {
    const rig = makeRig([], true);
    rig.store.toggle(1, 1);

    expect(rig.store.isBookmarked(1, 1)).toBe(true);
    expect(rig.store.isMarkedKey("1:1")).toBe(true);
    expect(rig.engine.enqueued).toHaveLength(1);
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

  it("toggle removes an existing bookmark and enqueues a bookmark.delete", () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);
    rig.store.toggle(2, 255);

    expect(rig.store.isBookmarked(2, 255)).toBe(false);
    expect(rig.store.bookmarks).toHaveLength(1);
    expect(rig.engine.enqueued).toHaveLength(1);
    expect(rig.engine.enqueued[0]!).toEqual({ kind: "bookmark.delete", id: "b1", updatedAt: expect.any(String) });
  });
});

describe("BookmarksStore — folders and moves", () => {
  it("createFolder enqueues folder.upsert and stores the trimmed name", () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("  Tafsir  ");

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

  it("renameFolder enqueues an upsert with the new name", () => {
    const rig = makeRig([], true);
    const folder = rig.store.createFolder("Old");
    if (!folder) throw new Error("folder missing");
    rig.store.renameFolder(folder.id, "New");

    expect(rig.store.folderById(folder.id)?.name).toBe("New");
    expect(rig.engine.enqueued[1]).toMatchObject({ kind: "folder.upsert", id: folder.id, name: "New" });
  });

  it("deleteFolder detaches children optimistically and enqueues folder.delete", () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.deleteFolder("f1");

    expect(rig.store.folders).toEqual([]);
    expect(rig.store.bookmarksIn(null)).toHaveLength(2);
    expect(rig.engine.enqueued).toEqual([{ kind: "folder.delete", id: "f1", updatedAt: expect.any(String) }]);
  });

  it("moveToFolder enqueues an upsert with the new folderId; unknown folder is a no-op", () => {
    const rig = makeRig([], true);
    rig.store.applyServer(SNAP);

    rig.store.moveToFolder("b2", "f1");
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
    rig.registered[0]!.applyServer(SNAP);
    expect(rig.store.bookmarks).toHaveLength(2);
  });

  it("logout clears the server view; re-login migrates only unseen legacy keys", async () => {
    const rig = makeRig(["2:255"], true);
      rig.store.hydrate();
    expect(rig.engine.enqueued).toHaveLength(1);

    rig.store.applyServer(SNAP);
    rig.setAuthed(false);
    rig.store.onAuthChanged(false);
    expect(rig.store.bookmarks).toEqual([]);
    expect(rig.store.folders).toEqual([]);

    // New legacy bookmark while signed out.
    rig.reader.setKeys(["2:255", "3:26"]);
    rig.setAuthed(true);
    rig.store.onAuthChanged(true);

    const kinds = rig.engine.enqueued.map((m) => `${m.kind}:${"surah" in m ? m.surah : ""}`);
    expect(kinds).toEqual(["bookmark.upsert:2", "bookmark.upsert:3"]);
  });
});
