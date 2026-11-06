import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import type { JsonRecord } from "#lib/storage/decoders.js";
import type { RegisteredSyncDomain, SyncStatus } from "#lib/sync/index.js";
import { SyncPausedError } from "#lib/sync/index.js";

import { defaultFolderIdFor } from "../schema";
import { BookmarksStore, type SyncEngineLike } from "../store.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

type Row = JsonRecord;

interface FakeAuth {
  authenticated: boolean;
  user: { id: number } | null;
  notifySessionLost: () => void;
}

interface FakeResponse {
  readonly status?: number;
  readonly body?: Row;
  readonly during?: () => void;
  readonly message?: string;
}

const STATUS: SyncStatus = { phase: "idle", pending: 0, lastSyncAt: null, lastError: null };

/** Plain JSON: undefined fields gone, shape left to the row decoders. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- test helper over arbitrary fixtures; JSON round-trip is the point.
function plain<T>(value: unknown): T {
  return JSON.parse(JSON.stringify(value));
}

function roundBody(over: Row = {}) {
  return {
    serverTime: Date.now(),
    syncEpoch: "1",
    results: [],
    folders: [],
    bookmarks: [],
    rev: 1,
    latestRev: 1,
    more: false,
    tombstoneFloor: 0,
    ...over,
  };
}

function setup(auth: FakeAuth = { authenticated: false, user: null, notifySessionLost: vi.fn() }) {
  const registered: RegisteredSyncDomain[] = [];
  const bodies: Row[] = [];
  const answers: Array<FakeResponse | "throw"> = [];
  const respond = (body: Row): AuthRequestResult<never> => {
    const next = answers.shift();
    if (next === "throw") throw new Error("connection lost");
    next?.during?.();
    const status = next?.status ?? 200;
    const mutations: Row[] = Array.isArray(body.mutations) ? plain(body.mutations) : [];
    const results: Row[] = plain(
      mutations.map((m) => ({
        id: m.id,
        status: "applied",
        stamps:
          m.kind === "bookmark.upsert"
            ? { row: m.stamp }
            : { name: m.nameStamp, position: m.positionStamp, deleted: m.deletedStamp },
      })),
    );
    return {
      ok: status < 400,
      status,
      // SAFETY: test double — the domain decodes this body as unknown.
      data: (status < 400 ? (next?.body ?? roundBody({ results })) : null) as never,
      error: next?.message === undefined ? null : { message: next.message },
      rotated: false,
    };
  };
  const engine: SyncEngineLike = {
    flush: async () => undefined,
    start: () => () => undefined,
    status: STATUS,
  };
  const store = new BookmarksStore({
    engine,
    auth: {
      unsafeRequest: async <T>(_path: string, init: UnsafeRequestInit) => {
        // SAFETY: test double — the domain always sends an object body.
        const body = init.body as Row;
        bodies.push(body);
        // SAFETY: test double — `never` data is assignable to whatever the domain asks for.
        return respond(body) as AuthRequestResult<T>;
      },
    },
    state: auth,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  store.hydrate();
  const round = async () => {
    const domain = registered.at(-1);
    if (domain?.round === undefined) throw new Error("no row domain");
    return domain.round();
  };
  return { store, registered, auth, bodies, answers, round };
}

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
  const FOLDER = "a1a1a1a1-a1a1-7a1a-8a1a-a1a1a1a1a1a1";
  const MARK = "b1b1b1b1-b1b1-7b1b-8b1b-b1b1b1b1b1b1";
  const MARK2 = "b2b2b2b2-b2b2-7b2b-8b2b-b2b2b2b2b2b2";
  const stamp = (ms: number, device = "other"): string =>
    `${String(ms).padStart(13, "0")}-0000-${device}`;
  const NOW = Date.now();

  const serverMark = (over: Row = {}) => ({
    id: MARK,
    folderId: FOLDER,
    surah: 2,
    ayah: 255,
    endSurah: 2,
    endAyah: 255,
    deleted: false,
    stamp: stamp(NOW),
    rev: 2,
    sourceId: null,
    createdAt: "2026-11-01T00:00:00.000Z",
    updatedAt: "2026-11-01T00:00:00.000Z",
    ...over,
  });
  const serverFolder = (over: Row = {}) => ({
    id: FOLDER,
    name: "Hifz",
    isDefault: false,
    position: null,
    deleted: false,
    rev: 1,
    updatedAt: "2026-11-01T00:00:00.000Z",
    nameStamp: stamp(NOW),
    positionStamp: null,
    deletedStamp: null,
    ...over,
  });

  it("edits apply at once, persist dirty with the row, and an ack clears them", async () => {
    const { store, registered, bodies, round } = setup(account());
    expect(registered.map((d) => d.name)).toEqual(["bookmarks.user:7"]);
    const toggled = store.toggle(2, 255);
    expect(toggled.kind).toBe("added");
    expect(store.isBookmarked(2, 255)).toBe(true);
    expect(store.pending).toBe(1);
    const cached = JSON.parse(localStorage.getItem("easyquran.bookmarks.user.7") ?? "{}");
    expect(cached.bookmarks[0]).toMatchObject({ dirty: true, surah: 2 });
    await round();
    expect(bodies[0]).toMatchObject({ expectedUserId: 7, client: { protocol: 1 } });
    expect(store.pending).toBe(0);
    expect(store.isBookmarked(2, 255)).toBe(true);
  });

  it("a kill before the ack loses nothing", () => {
    const first = setup(account());
    first.store.toggle(2, 255);
    const second = setup(account());
    expect(second.store.pending).toBe(1);
    expect(second.store.isBookmarked(2, 255)).toBe(true);
  });

  it("an edit while the request flies stays dirty", async () => {
    const { store, answers, round } = setup(account());
    store.toggle(2, 255);
    answers.push({ during: () => store.toggle(2, 255) });
    await round();
    expect(store.pending).toBe(1);
    expect(store.isBookmarked(2, 255)).toBe(false);
  });

  it("a delete pulled from another device reaches this one, and an empty pull deletes nothing", async () => {
    const { store, answers, round } = setup(account());
    answers.push({
      body: roundBody({
        folders: [serverFolder()],
        bookmarks: [serverMark()],
        rev: 2,
        latestRev: 2,
      }),
    });
    await round();
    expect(store.isBookmarked(2, 255)).toBe(true);
    answers.push({ body: roundBody({ rev: 2, latestRev: 2 }) });
    await round();
    expect(store.isBookmarked(2, 255)).toBe(true);
    answers.push({
      body: roundBody({
        bookmarks: [
          {
            id: MARK,
            folderId: FOLDER,
            surah: 2,
            ayah: 255,
            endSurah: 2,
            endAyah: 255,
            deleted: true,
            stamp: stamp(NOW + 10),
            rev: 3,
          },
        ],
        rev: 3,
        latestRev: 3,
      }),
    });
    await round();
    expect(store.isBookmarked(2, 255)).toBe(false);
  });

  it("two ids for one key show one row, and converge when the loser's tombstone lands", async () => {
    const { store, answers, round } = setup(account());
    answers.push({
      body: roundBody({
        folders: [serverFolder()],
        bookmarks: [serverMark(), serverMark({ id: MARK2, stamp: stamp(NOW + 5) })],
        rev: 3,
        latestRev: 3,
      }),
    });
    await round();
    expect(store.bookmarks.map((b) => b.id)).toEqual([MARK2]);
  });

  it("rename here and move there both survive", async () => {
    const { store, answers, round } = setup(account());
    answers.push({ body: roundBody({ folders: [serverFolder()], rev: 1, latestRev: 1 }) });
    await round();
    store.renameFolder(FOLDER, "Memorise");
    answers.push({
      body: roundBody({
        folders: [serverFolder({ position: 9, positionStamp: stamp(NOW + 1), rev: 2 })],
        rev: 2,
        latestRev: 2,
      }),
    });
    await round();
    expect(store.folderById(FOLDER)).toMatchObject({ name: "Memorise", position: 9 });
  });

  it("folder delete: older bookmarks die, newer ones show in the default folder", async () => {
    const { store, answers, round } = setup(account());
    answers.push({
      body: roundBody({
        folders: [serverFolder()],
        bookmarks: [
          serverMark({ stamp: stamp(NOW - 100) }),
          serverMark({ id: MARK2, ayah: 1, endAyah: 1, stamp: stamp(NOW + 100) }),
        ],
        rev: 3,
        latestRev: 3,
      }),
    });
    await round();
    answers.push({
      body: roundBody({
        folders: [
          {
            id: FOLDER,
            deleted: true,
            rev: 4,
            nameStamp: null,
            positionStamp: null,
            deletedStamp: stamp(NOW),
          },
        ],
        rev: 4,
        latestRev: 4,
      }),
    });
    await round();
    expect(store.folderById(FOLDER)).toBeUndefined();
    expect(store.bookmarks.map((b) => b.id)).toEqual([MARK2]);
    expect(store.bookmarks[0]?.folderId).toBe(defaultFolderIdFor(7));
  });

  it("deleting a folder here hides it at once, and undo restores with newer stamps", () => {
    const { store } = setup(account());
    const folder = store.createFolder("Hifz");
    expect(folder).not.toBeNull();
    if (folder === null) return;
    store.setInFolder(2, 255, folder.id, true);
    const deleted = store.deleteFolder(folder.id);
    expect(store.folderById(folder.id)).toBeUndefined();
    expect(store.isBookmarked(2, 255)).toBe(false);
    if (deleted === null) throw new Error("expected a delete");
    store.restoreFolder(deleted);
    expect(store.folderById(folder.id)).toBeDefined();
    expect(store.isBookmarked(2, 255)).toBe(true);
  });

  it("rejected keeps the row and stops resending; capped stays dirty and is surfaced", async () => {
    const { store, answers, round } = setup(account());
    store.toggle(2, 255);
    const id = store.bookmarks[0]?.id ?? "";
    answers.push({ body: roundBody({ results: [{ id, status: "rejected" }] }) });
    await round();
    expect(store.rejectedCount).toBe(1);
    expect(store.dirtyRows()).toEqual([]);
    expect(store.isBookmarked(2, 255)).toBe(true);

    store.toggle(3, 1);
    const other = store.bookmarks.find((b) => b.surah === 3)?.id ?? "";
    answers.push({ body: roundBody({ results: [{ id: other, status: "capped" }] }) });
    await round();
    expect(store.cappedCount).toBe(1);
    expect(store.dirtyRows().map((r) => r.id)).toEqual([other]);
  });

  it("past the floor: full pull, and a stale device revives nothing", async () => {
    const { store, answers, round, bodies } = setup(account());
    answers.push({
      body: roundBody({
        folders: [serverFolder()],
        bookmarks: [serverMark()],
        rev: 5,
        latestRev: 5,
      }),
    });
    await round();
    answers.push({ body: roundBody({ rev: 50, latestRev: 50, tombstoneFloor: 40 }) });
    expect((await round()).more).toBe(true);
    answers.push({ body: roundBody({ rev: 50, latestRev: 50, tombstoneFloor: 40 }) });
    await round();
    expect(bodies.at(-1)).toMatchObject({ since: 0 });
    expect(store.isBookmarked(2, 255)).toBe(false);
  });

  it("a restored server forces a full pull and re-push", async () => {
    const { store, answers, round } = setup(account());
    store.toggle(2, 255);
    await round();
    expect(store.pending).toBe(0);
    answers.push({ body: roundBody({ syncEpoch: "2" }) });
    expect((await round()).more).toBe(true);
    expect(store.pending).toBe(1);
  });

  it("signing in folds device bookmarks into the account, device default → account default", () => {
    const auth: FakeAuth = { authenticated: false, user: null, notifySessionLost: vi.fn() };
    const seen: number[] = [];
    const { store } = setup(auth);
    store.onMerged((count) => seen.push(count));
    store.toggle(2, 255);
    auth.authenticated = true;
    auth.user = { id: 7 };
    store.onAuthChanged();
    expect(seen).toEqual([1]);
    expect(localStorage.getItem("easyquran.bookmarks.local")).toBeNull();
    expect(store.bookmarks[0]?.folderId).toBe(defaultFolderIdFor(7));
    const rows = store.dirtyRows();
    expect(rows[0]).toMatchObject({
      kind: "bookmark.upsert",
      stamp: expect.stringMatching(/^\d{13}-/u),
    });
  });

  it("an account switch never shows the previous account's rows", () => {
    const auth = account();
    const { store } = setup(auth);
    store.toggle(2, 255);
    auth.user = { id: 9 };
    store.onAuthChanged();
    expect(store.bookmarks).toEqual([]);
    expect(store.dirtyRows()).toEqual([]);
    auth.user = { id: 7 };
    store.onAuthChanged();
    expect(store.isBookmarked(2, 255)).toBe(true);
    expect(store.pending).toBe(1);
  });

  it("a cookie-account mismatch suspends the view and hands the loss to the auth seam", async () => {
    const auth = account();
    const { store, answers, round } = setup(auth);
    answers.push({ status: 403, message: "Bookmark sync account changed" });
    await expect(round()).rejects.toBeInstanceOf(SyncPausedError);
    expect(store.authed).toBe(false);
    expect(auth.notifySessionLost).toHaveBeenCalledOnce();
  });
});

describe("bulk actions (F14)", () => {
  const account = (): FakeAuth => ({
    authenticated: true,
    user: { id: 7 },
    notifySessionLost: vi.fn(),
  });

  it("signed out: bulk move then Undo restores every row to its old folder", () => {
    const { store } = setup();
    const hifz = store.createFolder("Hifz")!;
    store.toggle(2, 1);
    store.toggle(2, 2);
    const ids = store.bookmarks.map((b) => b.id);
    const moved = store.moveMany(ids, hifz.id);
    expect(moved).toHaveLength(2);
    expect(store.bookmarksIn(hifz.id)).toHaveLength(2);
    store.restore(moved);
    expect(store.bookmarksIn(store.defaultFolderId)).toHaveLength(2);
    expect(store.bookmarksIn(hifz.id)).toHaveLength(0);
  });

  it("signed out: bulk remove then Undo brings the rows back", () => {
    const { store } = setup();
    store.toggle(2, 1);
    store.toggle(2, 2);
    store.toggle(2, 3);
    const removed = store.removeMany([store.bookmarks[0]!.id, store.bookmarks[1]!.id]);
    expect(removed).toHaveLength(2);
    expect(store.bookmarks).toHaveLength(1);
    store.restore(removed);
    expect(store.bookmarks).toHaveLength(3);
  });

  it("bulk move skips rows already in the target and unknown ids", () => {
    const { store } = setup();
    const hifz = store.createFolder("Hifz")!;
    store.toggle(5, 1);
    const id = store.bookmarks[0]!.id;
    expect(store.moveMany([id, "missing-id"], store.defaultFolderId)).toEqual([]);
    expect(store.moveMany([id, "missing-id"], "no-such-folder")).toEqual([]);
    expect(store.moveMany([id], hifz.id)).toHaveLength(1);
  });

  it("signed in: each bulk row is stamped and dirty, and Undo writes newer stamps", () => {
    const { store } = setup(account());
    store.toggle(2, 1);
    store.toggle(2, 2);
    const before = store.bookmarks.map((b) => b.stamp);
    const removed = store.removeMany(store.bookmarks.map((b) => b.id));
    expect(store.bookmarks).toHaveLength(0);
    expect(store.pending).toBeGreaterThan(0);
    store.restore(removed);
    expect(store.bookmarks).toHaveLength(2);
    expect(store.bookmarks.every((b) => b.dirty)).toBe(true);
    for (const [index, row] of store.bookmarks.entries()) {
      const prior = before[index];
      expect(prior).not.toBeNull();
      expect(row.stamp! > prior!).toBe(true);
    }
  });

  it("signed in: bulk move writes dirty rows in the new folder; Undo returns them", () => {
    const { store } = setup(account());
    const hifz = store.createFolder("Hifz")!;
    store.toggle(3, 1);
    store.toggle(3, 2);
    const moved = store.moveMany(
      store.bookmarks.map((b) => b.id),
      hifz.id,
    );
    expect(store.bookmarksIn(hifz.id)).toHaveLength(2);
    expect(store.bookmarks.every((b) => b.dirty)).toBe(true);
    store.restore(moved);
    expect(store.bookmarksIn(store.defaultFolderId)).toHaveLength(2);
  });
});
