import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { RegisteredSyncDomain, SyncMutation, SyncStatus } from "#lib/sync/index.js";

import { defaultFolderIdFor, type BookmarksMutation, type BookmarksSnapshot } from "../schema";
import { BookmarksStore, type SyncEngineLike } from "../store.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

interface FakeAuth {
  authenticated: boolean;
  user: { id: number } | null;
  notifySessionLost: () => void;
}

const STATUS: SyncStatus = { phase: "idle", pending: 0, lastSyncAt: null, lastError: null };

function setup(auth: FakeAuth = { authenticated: false, user: null, notifySessionLost: vi.fn() }) {
  const enqueued: Array<{ domain: string; payload: BookmarksMutation }> = [];
  const registered: RegisteredSyncDomain[] = [];
  const engine: SyncEngineLike = {
    async enqueue<P>(domain: string, payload: P) {
      // SAFETY: test double — BookmarksStore only enqueues BookmarksMutation payloads.
      enqueued.push({ domain, payload: payload as BookmarksMutation });
      // SAFETY: test double — a minimal SyncMutation; the store reads id only.
      return {
        id: `m${enqueued.length}`,
        domain,
        seq: enqueued.length,
        payload,
        queuedAt: 0,
      } as SyncMutation<P>;
    },
    pendingFor: () => 0,
    flush: async () => undefined,
    start: () => () => undefined,
    status: STATUS,
  };
  const store = new BookmarksStore({
    engine,
    auth: {
      unsafeRequest: async () => ({
        ok: true,
        status: 200,
        data: null,
        error: null,
        rotated: false,
      }),
    },
    state: auth,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  store.hydrate();
  return { store, enqueued, registered, auth };
}

/** Let the store's async enqueue handlers settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  window.localStorage.clear();
});

describe("signed out: device folders", () => {
  it("starts with the default folder and saves a tap there", () => {
    const { store } = setup();
    expect(store.folders.map((f) => f.isDefault)).toEqual([true]);
    expect(store.defaultFolderId).toBe(defaultFolderIdFor("local"));
    const result = store.toggle(2, 255, "en.sahih");
    expect(result).toMatchObject({ kind: "added", folder: { isDefault: true } });
    expect(store.isBookmarked(2, 255)).toBe(true);
    expect(store.bookmarks[0]).toMatchObject({ sourceId: "en.sahih" });
    // Persisted: a fresh store reads it back.
    expect(setup().store.isBookmarked(2, 255)).toBe(true);
  });

  it("a verse can sit in several folders; a second tap removes it everywhere and Undo restores", () => {
    const { store } = setup();
    const hifz = store.createFolder("  Hifz ");
    expect(hifz?.name).toBe("Hifz");
    store.toggle(36, 1);
    store.setInFolder(36, 1, hifz!.id, true);
    expect(store.foldersOf(36, 1).size).toBe(2);
    const removed = store.toggle(36, 1);
    expect(removed.kind).toBe("removed");
    expect(store.isBookmarked(36, 1)).toBe(false);
    if (removed.kind === "removed") store.restore(removed.bookmarks);
    expect(store.foldersOf(36, 1).size).toBe(2);
  });

  it("taps go to the last folder used", () => {
    const { store } = setup();
    const hifz = store.createFolder("Hifz")!;
    store.toggle(1, 1);
    store.setInFolder(1, 1, hifz.id, true);
    expect(store.targetFolder.id).toBe(hifz.id);
    expect(store.toggle(1, 2)).toMatchObject({ kind: "added", folder: { id: hifz.id } });
  });

  it("deleting a folder takes its bookmarks; the default folder never goes; Undo restores both", () => {
    const { store } = setup();
    const hifz = store.createFolder("Hifz")!;
    store.toggle(2, 1);
    store.setInFolder(2, 2, hifz.id, true);
    expect(store.deleteFolder(store.defaultFolderId)).toBeNull();
    const deleted = store.deleteFolder(hifz.id);
    expect(deleted?.bookmarks).toHaveLength(1);
    expect(store.folders).toHaveLength(1);
    expect(store.isBookmarked(2, 2)).toBe(false);
    store.restoreFolder(deleted!);
    expect(store.folders).toHaveLength(2);
    expect(store.isBookmarked(2, 2)).toBe(true);
  });

  it("moving onto a folder that already holds the verse collapses into one row", () => {
    const { store } = setup();
    const hifz = store.createFolder("Hifz")!;
    store.toggle(3, 3);
    store.setInFolder(3, 3, hifz.id, true);
    const inDefault = store.rowsFor(3, 3).find((b) => b.folderId === store.defaultFolderId)!;
    store.moveToFolder(inDefault.id, hifz.id);
    expect(store.rowsFor(3, 3).map((b) => b.folderId)).toEqual([hifz.id]);
  });

  it("snippet ranges toggle as range rows in the default folder", () => {
    const { store } = setup();
    store.toggleRange(2, 285, 2, 286);
    expect(store.isRangeMarked(2, 285, 2, 286)).toBe(true);
    expect(store.isBookmarked(2, 285)).toBe(false);
    store.toggleRange(2, 285, 2, 286);
    expect(store.isRangeMarked(2, 285, 2, 286)).toBe(false);
  });
});

describe("signed in: synced folders", () => {
  const account = (): FakeAuth => ({
    authenticated: true,
    user: { id: 7 },
    notifySessionLost: vi.fn(),
  });

  it("edits apply optimistically and queue in the account partition", async () => {
    const { store, enqueued, registered } = setup(account());
    expect(registered.map((d) => d.name)).toEqual(["bookmarks.user:7"]);
    expect(store.defaultFolderId).toBe(defaultFolderIdFor(7));
    store.toggle(2, 255);
    const folder = store.createFolder("Hifz")!;
    store.setInFolder(2, 255, folder.id, true);
    await settle();
    expect(store.foldersOf(2, 255).size).toBe(2);
    expect(enqueued.map((e) => `${e.domain} ${e.payload.kind}`)).toEqual([
      "bookmarks.user:7 bookmark.upsert",
      "bookmarks.user:7 folder.upsert",
      "bookmarks.user:7 bookmark.upsert",
    ]);
    expect(enqueued[0]?.payload).toMatchObject({ folderId: defaultFolderIdFor(7), sourceId: null });
  });

  it("pending rows beat a stale snapshot until their mutation drains", async () => {
    const { store, enqueued } = setup(account());
    store.toggle(1, 1);
    await settle();
    const stale: BookmarksSnapshot = { folders: [], bookmarks: [] };
    store.applyServer(stale);
    expect(store.isBookmarked(1, 1)).toBe(true);
    const row = store.bookmarks[0]!;
    const drained = [
      {
        id: "m1",
        domain: "bookmarks.user:7",
        seq: 1,
        queuedAt: 0,
        payload: enqueued[0]!.payload,
      },
    ];
    store.applyServer({ folders: [], bookmarks: [row] }, drained);
    expect(store.bookmarks.map((b) => b.id)).toEqual([row.id]);
    store.applyServer({ folders: [], bookmarks: [] });
    expect(store.isBookmarked(1, 1)).toBe(false);
  });

  it("a folder delete hides its rows at once (the server cascades)", async () => {
    const { store, enqueued } = setup(account());
    const folder = store.createFolder("Juz Amma")!;
    store.setInFolder(78, 1, folder.id, true);
    store.deleteFolder(folder.id);
    await settle();
    expect(store.isBookmarked(78, 1)).toBe(false);
    expect(enqueued.at(-1)?.payload).toMatchObject({ kind: "folder.delete", id: folder.id });
  });

  it("signing in folds device bookmarks into the account, device default → account default", async () => {
    const auth: FakeAuth = { authenticated: false, user: null, notifySessionLost: vi.fn() };
    const { store, enqueued } = setup(auth);
    const merged = vi.fn();
    store.onMerged(merged);
    const hifz = store.createFolder("Hifz")!;
    store.toggle(2, 255);
    store.setInFolder(18, 10, hifz.id, true);
    auth.authenticated = true;
    auth.user = { id: 7 };
    store.onAuthChanged();
    await settle();
    expect(merged).toHaveBeenCalledWith(2);
    expect(window.localStorage.getItem("easyquran.bookmarks.local")).toBeNull();
    const upserts = enqueued
      .filter((e) => e.payload.kind === "bookmark.upsert")
      .map((e) => e.payload);
    expect(upserts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ surah: 2, ayah: 255, folderId: defaultFolderIdFor(7) }),
        expect.objectContaining({ surah: 18, ayah: 10, folderId: hifz.id }),
      ]),
    );
    expect(store.isBookmarked(2, 255)).toBe(true);
  });

  it("an account switch never shows the previous account's rows", async () => {
    const auth = account();
    const { store } = setup(auth);
    store.toggle(5, 5);
    await settle();
    auth.user = { id: 8 };
    store.onAuthChanged();
    expect(store.isBookmarked(5, 5)).toBe(false);
    expect(store.defaultFolderId).toBe(defaultFolderIdFor(8));
  });

  it("a cookie-account mismatch suspends the view and hands the loss to the auth seam", async () => {
    const auth = account();
    const { store, registered } = setup(auth);
    const domain = registered[0]!;
    // The domain reports the mismatch through its callback, then pauses.
    const failing = new BookmarksStore({
      engine: {
        enqueue: async <P>(domainName: string, payload: P) =>
          // SAFETY: test double.
          ({ id: "x", domain: domainName, seq: 1, payload, queuedAt: 0 }) as SyncMutation<P>,
        pendingFor: () => 0,
        flush: async () => undefined,
        start: () => () => undefined,
        status: STATUS,
      },
      auth: {
        unsafeRequest: async () => ({
          ok: false,
          status: 403,
          data: null,
          error: { message: "Bookmark sync account changed" },
          rotated: false,
        }),
      },
      state: auth,
      register: (d) => registered.push(d),
      unregister: () => undefined,
    });
    failing.hydrate();
    const adapter = registered.at(-1)!;
    await expect(adapter.sync([])).rejects.toThrow("paused");
    expect(failing.authed).toBe(false);
    expect(auth.notifySessionLost).toHaveBeenCalled();
    expect(domain.name).toBe("bookmarks.user:7");
    expect(store.authed).toBe(true);
  });
});
