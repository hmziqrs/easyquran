import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import { createSyncEngine } from "#lib/sync/engine.svelte.js";
import { createOutbox, memoryQueueStorage, type QueueStorage } from "#lib/sync/outbox.js";

import { bookmarksDomainFor, type BookmarkAuthLike } from "../domain";
import type { BookmarksMutation, BookmarksSnapshot } from "../schema";
import { createBookmarksStore } from "../store.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

const stops: Array<() => void> = [];
const emptySnapshot: BookmarksSnapshot = { folders: [], bookmarks: [] };

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function success(snapshot = emptySnapshot): AuthRequestResult<unknown> {
  return { ok: true, status: 200, data: { applied: 0, ...snapshot }, error: null, rotated: false };
}

function snapshotFor(id: string, surah: number): BookmarksSnapshot {
  return {
    folders: [],
    bookmarks: [
      {
        id,
        folderId: null,
        surah,
        ayah: 1,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    ],
  };
}

function queuedBookmark(id: string, surah: number): BookmarksMutation {
  return {
    kind: "bookmark.upsert",
    id,
    folderId: null,
    surah,
    ayah: 1,
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

interface MutableAuthState {
  authenticated: boolean;
  user: { id: number } | null;
}

interface SyncBody {
  readonly expectedUserId: number;
  readonly mutations: BookmarksMutation[];
}

function makeRig(opts: {
  ownerId: number;
  storage?: QueueStorage;
  online?: boolean;
  legacyKeys?: string[];
  respond?: (body: SyncBody, init: UnsafeRequestInit) => Promise<AuthRequestResult<unknown>>;
}) {
  const state: MutableAuthState = {
    authenticated: true,
    user: { id: opts.ownerId },
  };
  const calls: SyncBody[] = [];
  const outbox = createOutbox(opts.storage ?? memoryQueueStorage());
  let online = opts.online ?? true;
  const engine = createSyncEngine({ outbox, online: () => online, flushDelayMs: 60_000 });
  stops.push(engine.start());
  const auth: BookmarkAuthLike = {
    async unsafeRequest<T>(_path: string, init: UnsafeRequestInit): Promise<AuthRequestResult<T>> {
      // SAFETY: createBookmarksDomain is the only caller; its sync payload has this owner and mutation shape.
      const body = init.body as SyncBody;
      calls.push(body);
      const result = opts.respond ? await opts.respond(body, init) : success();
      // SAFETY: this fake returns only fixture envelopes; createBookmarksDomain requests unknown then validates their body.
      return result as AuthRequestResult<T>;
    },
  };
  const store = createBookmarksStore({
    engine,
    auth,
    state,
    reader: {
      isBookmarked: () => false,
      toggleBookmark: () => undefined,
      bookmarkedKeys: opts.legacyKeys ?? [],
    },
    register: (domain) => engine.register(domain),
    unregister: (name) => engine.unregister(name),
  });
  return {
    store,
    engine,
    outbox,
    calls,
    setOnline(value: boolean) {
      online = value;
    },
    setOwner(ownerId: number | null) {
      state.authenticated = ownerId !== null;
      state.user = ownerId === null ? null : { id: ownerId };
      store.onAuthChanged(state.authenticated);
    },
  };
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
});

describe("account-owned bookmark synchronization", () => {
  it("preserves pending A mutations while B syncs only B's partition", async () => {
    const rig = makeRig({ ownerId: 7, online: false });
    rig.store.hydrate();
    rig.store.toggle(1, 1);
    await vi.waitFor(async () => expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1));
    rig.setOwner(9);
    rig.store.toggle(2, 1);
    await vi.waitFor(async () => expect(await rig.outbox.count(bookmarksDomainFor(9))).toBe(1));
    rig.setOnline(true);
    await rig.engine.flush();
    expect(rig.calls).toHaveLength(1);
    expect(rig.calls[0]).toMatchObject({ expectedUserId: 9, mutations: [{ surah: 2 }] });
    expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1);
    expect(await rig.outbox.count(bookmarksDomainFor(9))).toBe(0);
    expect(rig.store.pending).toBe(0);
  });

  it("ignores delayed A responses and coalesces B's requested flush", async () => {
    const responseA = deferred<AuthRequestResult<unknown>>();
    let signalA: AbortSignal | undefined;
    const rig = makeRig({
      ownerId: 7,
      respond: async (body, init) => {
        if (body.expectedUserId === 7) {
          signalA = init.signal;
          return await responseA.promise;
        }
        return success(snapshotFor("b", 2));
      },
    });
    await rig.outbox.enqueue(bookmarksDomainFor(7), queuedBookmark("a", 1));
    rig.store.hydrate();
    await vi.waitFor(() => expect(rig.calls).toHaveLength(1));
    rig.setOwner(9);
    expect(signalA?.aborted).toBe(true);
    responseA.resolve(success(snapshotFor("a", 1)));
    await vi.waitFor(() =>
      expect(rig.store.bookmarks.map((bookmark) => bookmark.id)).toEqual(["b"]),
    );
    expect(rig.calls.map((call) => call.expectedUserId)).toEqual([7, 9]);
    expect(rig.calls[1]?.mutations).toEqual([]);
    expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1);
  });

  it("logout and same-account re-login invalidate the old generation", async () => {
    const oldResponse = deferred<AuthRequestResult<unknown>>();
    const newResponse = deferred<AuthRequestResult<unknown>>();
    let requests = 0;
    const rig = makeRig({
      ownerId: 7,
      respond: async () => {
        requests += 1;
        if (requests === 1) return await oldResponse.promise;
        return await newResponse.promise;
      },
    });
    rig.store.hydrate();
    await vi.waitFor(() => expect(rig.calls).toHaveLength(1));
    rig.store.suspendAuth();
    rig.setOwner(null);
    rig.setOwner(7);
    oldResponse.resolve(success(snapshotFor("old", 1)));
    await vi.waitFor(() => expect(rig.calls).toHaveLength(2));
    expect(rig.store.bookmarks).toEqual([]);
    newResponse.resolve(success(snapshotFor("fresh", 2)));
    await vi.waitFor(() =>
      expect(rig.store.bookmarks.map((bookmark) => bookmark.id)).toEqual(["fresh"]),
    );
  });

  it("cold reload selects B's durable queue, discards unowned rows, preserves A for A's next login", async () => {
    const storage = memoryQueueStorage();
    const seed = createOutbox(storage);
    await seed.enqueue("bookmarks", queuedBookmark("unknown", 3));
    await seed.enqueue(bookmarksDomainFor(7), queuedBookmark("a", 1));
    await seed.enqueue(bookmarksDomainFor(9), queuedBookmark("b", 2));
    const first = makeRig({ ownerId: 9, storage });
    first.store.hydrate();
    await vi.waitFor(() => expect(first.calls).toHaveLength(1));
    await first.engine.flush(bookmarksDomainFor(9));
    expect(first.calls[0]).toMatchObject({ expectedUserId: 9, mutations: [{ id: "b" }] });
    expect(await seed.count("bookmarks")).toBe(0);
    expect(await seed.count(bookmarksDomainFor(7))).toBe(1);
    expect(await seed.count(bookmarksDomainFor(9))).toBe(0);
    first.setOwner(null);
    const reload = makeRig({ ownerId: 7, storage });
    reload.store.hydrate();
    await vi.waitFor(() => expect(reload.calls).toHaveLength(1));
    expect(reload.calls[0]).toMatchObject({ expectedUserId: 7, mutations: [{ id: "a" }] });
  });

  it("durability completion after A→B cannot apply A's optimistic row into B", async () => {
    const gate = deferred<void>();
    const storage = memoryQueueStorage();
    let blocked = true;
    const delayed: QueueStorage = {
      ...storage,
      async write(record) {
        if (blocked) {
          blocked = false;
          await gate.promise;
        }
        return await storage.write(record);
      },
    };
    const rig = makeRig({ ownerId: 7, storage: delayed, online: false });
    rig.store.hydrate();
    rig.store.toggle(1, 1);
    rig.setOwner(9);
    rig.store.toggle(2, 1);
    gate.resolve();
    await vi.waitFor(() =>
      expect(rig.store.bookmarks.map((bookmark) => bookmark.surah)).toEqual([2]),
    );
    expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1);
    expect(await rig.outbox.count(bookmarksDomainFor(9))).toBe(1);
  });

  it("account switch during guest migration stops A's work and never marks A complete", async () => {
    const gate = deferred<void>();
    const storage = memoryQueueStorage();
    let blocked = true;
    let writingA = false;
    const delayed: QueueStorage = {
      ...storage,
      async write(record) {
        if (blocked) {
          blocked = false;
          writingA = true;
          await gate.promise;
        }
        return await storage.write(record);
      },
    };
    const rig = makeRig({
      ownerId: 7,
      storage: delayed,
      online: false,
      legacyKeys: ["1:1", "2:1"],
    });
    rig.store.hydrate();
    await vi.waitFor(() => expect(writingA).toBe(true));
    rig.setOwner(9);
    gate.resolve();
    await vi.waitFor(() =>
      expect(localStorage.getItem("eq.bookmarks.legacy-migrated.9")).toBe("1"),
    );
    expect(localStorage.getItem("eq.bookmarks.legacy-migrated.7")).toBeNull();
    expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1);
    expect(await rig.outbox.count(bookmarksDomainFor(9))).toBe(2);
  });

  it("stale tab sends captured owner so a different cookie account cannot accept its mutations", async () => {
    const cookieOwnerId = 9;
    const accepted: BookmarksMutation[] = [];
    const rig = makeRig({
      ownerId: 7,
      respond: async (body) => {
        if (body.expectedUserId !== cookieOwnerId) {
          return { ok: false, status: 409, data: null, error: null, rotated: false };
        }
        accepted.push(...body.mutations);
        return success();
      },
    });
    await rig.outbox.enqueue(bookmarksDomainFor(7), queuedBookmark("a", 1));
    rig.store.hydrate();
    await vi.waitFor(() => expect(rig.calls).toHaveLength(1));
    await rig.engine.flush();
    expect(rig.calls.every((call) => call.expectedUserId === 7)).toBe(true);
    expect(accepted).toEqual([]);
    expect(await rig.outbox.count(bookmarksDomainFor(7))).toBe(1);
    expect(rig.store.bookmarks).toEqual([]);
  });

  it("clears an already loaded A view when the server reports a cookie/account mismatch", async () => {
    const rig = makeRig({
      ownerId: 7,
      respond: async () => ({
        ok: false,
        status: 403,
        data: null,
        error: { message: "Bookmark sync account changed" },
        rotated: false,
      }),
    });
    rig.store.applyServer(snapshotFor("private-a", 1));
    expect(rig.store.bookmarks).toHaveLength(1);
    rig.store.hydrate();
    await vi.waitFor(() => expect(rig.calls).toHaveLength(1));
    await rig.engine.flush();
    expect(rig.store.bookmarks).toEqual([]);
    expect(rig.store.authed).toBe(false);
  });
});
