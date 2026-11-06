import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { RegisteredSyncDomain } from "#lib/sync/index.js";

import { markEntry, noteLeaving } from "../entry";
import { STALE_AFTER_MS, decodeEnvelope } from "../schema";
import { SessionsStore, type ReaderTarget, type SessionsEngineLike } from "../store.svelte";

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
}

function fakeEngine() {
  const flushed: Array<string | undefined> = [];
  const engine: SessionsEngineLike = {
    async flush(domain) {
      flushed.push(domain);
    },
    start: () => () => undefined,
  };
  return { engine, flushed };
}

function makeStore(auth: FakeAuth, engine = fakeEngine()) {
  const registered: RegisteredSyncDomain[] = [];
  const store = new SessionsStore({
    engine: engine.engine,
    auth: {
      unsafeRequest: async () => ({
        ok: true,
        status: 200,
        data: null,
        error: null,
        rotated: false,
      }),
      // Archive pages are only fetched for accounts; these tests never page one.
      get: async <T>(): Promise<{ ok: boolean; data?: T | null }> => ({ ok: false }),
    },
    state: auth,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  return { store, engine, registered };
}

const surah = (num: number, ayah = 1, exact = false): ReaderTarget => ({
  scope: `surah:${num}`,
  surah: num,
  ayah,
  exact,
  sourceId: null,
});

const anon = (): FakeAuth => ({ authenticated: false, user: null });

beforeEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
  noteLeaving(false);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("thread lifecycle", () => {
  it("an opened scope stays a draft until something is actually read", () => {
    const { store } = makeStore(anon());
    const { thread } = store.enter(surah(73));
    expect(store.current?.id).toBe(thread.id);
    expect(store.open).toEqual([]);
    // The reader reporting its first verse is no evidence of reading.
    store.cursor(73, 1, null);
    expect(store.open).toEqual([]);
    store.cursor(73, 4, null);
    expect(store.open.map((t) => t.id)).toEqual([thread.id]);
    expect(store.open[0]).toMatchObject({ origin: "link", scope: "surah:73", ayah: 4 });
  });

  it("a verse read in place materializes the draft and advances the cursor", () => {
    const { store } = makeStore(anon());
    store.enter(surah(1));
    store.noteRead(1, 3);
    expect(store.open[0]).toMatchObject({ surah: 1, ayah: 3 });
    expect(store.furthestIn(1)).toBe(3);
  });

  it("continuation keeps the thread: reload on the same scope, or untagged in-reader navigation", () => {
    const { store } = makeStore(anon());
    const first = store.enter(surah(2)).thread;
    store.cursor(2, 286, null);
    expect(store.enter(surah(2)).thread.id).toBe(first.id);
    noteLeaving(true);
    expect(store.enter(surah(3)).thread.id).toBe(first.id);
    store.cursor(3, 2, null);
    expect(store.open).toHaveLength(1);
    expect(store.open[0]).toMatchObject({ surah: 3, ayah: 2, scope: "surah:2" });
  });

  it("a whole-surah entry resumes the open thread there and offers Start over", () => {
    const { store } = makeStore(anon());
    const muzzammil = store.enter(surah(73)).thread;
    store.cursor(73, 12, null);
    markEntry({ origin: "index" });
    store.enter(surah(1));
    store.cursor(1, 5, null);
    markEntry({ origin: "index" });
    const back = store.enter(surah(73));
    expect(back).toMatchObject({ resumed: true });
    expect(back.thread.id).toBe(muzzammil.id);
    expect(store.offer).toBe(muzzammil.id);

    const fresh = store.startOver();
    expect(fresh?.id).not.toBe(muzzammil.id);
    expect(fresh).toMatchObject({ surah: 73, ayah: 1 });
    expect(store.offer).toBeNull();
    expect(store.open.some((t) => t.id === muzzammil.id)).toBe(false);
  });

  it("an exact-ayah entry (search, bookmark) always starts a new thread", () => {
    const { store } = makeStore(anon());
    const reading = store.enter(surah(73)).thread;
    store.cursor(73, 12, null);
    markEntry({ origin: "search", exact: true });
    const jump = store.enter(surah(73, 10, true));
    expect(jump.thread.id).not.toBe(reading.id);
    expect(jump.thread).toMatchObject({ origin: "search", ayah: 10 });
  });

  it("card and snippet entries are bounded and complete at their last verse", () => {
    const { store } = makeStore(anon());
    markEntry({ origin: "card", ref: "card-1", scope: "range:2:285-2:286", boundEnd: "2:286" });
    const { thread } = store.enter(surah(2, 285, true));
    expect(thread).toMatchObject({
      origin: "card",
      originRef: "card-1",
      boundEnd: "2:286",
      ayah: 285,
    });
    store.noteRead(2, 285);
    expect(store.open).toHaveLength(1);
    store.noteRead(2, 286);
    expect(store.open).toEqual([]);
  });

  it("an explicit resume reopens an ended thread", () => {
    const { store } = makeStore(anon());
    const { thread } = store.enter(surah(36));
    store.cursor(36, 9, null);
    store.dismiss(thread.id);
    expect(store.open).toEqual([]);
    store.markResume(thread.id);
    const back = store.enter(surah(36));
    expect(back.thread.id).toBe(thread.id);
    expect(store.open.map((t) => t.id)).toEqual([thread.id]);
  });

  it("history holds ended and stale threads; remove deletes for good", async () => {
    const { store } = makeStore(anon());
    const done = store.enter(surah(112)).thread;
    store.cursor(112, 4, null);
    store.dismiss(done.id);
    vi.useFakeTimers();
    vi.setSystemTime(Date.now() + STALE_AFTER_MS + 1000);
    markEntry({ origin: "index" });
    store.enter(surah(18));
    store.cursor(18, 2, null);
    await store.loadArchive(true);
    expect(store.archive.items.map((t) => t.id)).toEqual([done.id]);
    store.remove(done.id);
    expect(store.archive.items).toEqual([]);
  });

  it("clear-all tombstones every ended thread, keeps open ones, and Undo restores them", async () => {
    const { store } = makeStore(anon());
    const first = store.enter(surah(112)).thread;
    store.cursor(112, 4, null);
    store.dismiss(first.id);
    const second = store.enter(surah(18)).thread;
    store.cursor(18, 2, null);
    store.dismiss(second.id);
    const reading = store.enter(surah(1)).thread;
    store.cursor(1, 3, null);

    const cleared = await store.clearHistory();
    expect(cleared?.threads.map((t) => t.id).sort()).toEqual([first.id, second.id].sort());
    expect(store.archive).toMatchObject({ items: [], next: null, loaded: true });
    expect(store.open.map((t) => t.id)).toEqual([reading.id]);
    await store.loadArchive(true);
    expect(store.archive.items).toEqual([]);

    if (cleared === null) throw new Error("anon history is always complete");
    store.restoreHistory(cleared);
    expect(store.archive.items.map((t) => t.id).sort()).toEqual([first.id, second.id].sort());
    await store.loadArchive(true);
    expect(store.archive.items.map((t) => t.id).sort()).toEqual([first.id, second.id].sort());
    expect(store.open.map((t) => t.id)).toEqual([reading.id]);
  });

  it("clear-all with nothing ended clears nothing and still reports success", async () => {
    const { store } = makeStore(anon());
    store.enter(surah(36));
    store.cursor(36, 2, null);
    const cleared = await store.clearHistory();
    expect(cleared?.threads).toEqual([]);
    expect(store.open).toHaveLength(1);
  });

  it("undo is ignored once another account owns the store", async () => {
    const { store } = makeStore(anon());
    const done = store.enter(surah(112)).thread;
    store.cursor(112, 4, null);
    store.dismiss(done.id);
    const cleared = await store.clearHistory();
    if (cleared === null) throw new Error("anon history is always complete");
    const stale = { ...cleared, owner: "7" };
    store.restoreHistory(stale);
    expect(store.archive.items).toEqual([]);
  });

  it("threads written by another tab survive this tab's next write (merge-on-write)", () => {
    vi.useFakeTimers();
    const tabA = makeStore(anon()).store;
    tabA.enter(surah(2));
    tabA.cursor(2, 7, null);
    vi.advanceTimersByTime(500);
    window.sessionStorage.clear();
    const tabB = makeStore(anon()).store;
    markEntry({ origin: "index" });
    tabB.enter(surah(67));
    tabB.cursor(67, 3, null);
    vi.advanceTimersByTime(500);
    const stored = decodeEnvelope(
      JSON.parse(window.localStorage.getItem("easyquran.sessions.local") ?? "null"),
    );
    expect(stored?.threads.map((t) => t.surah).sort((a, b) => a - b)).toEqual([2, 67]);
  });

  it("a cursor move names the install that made it, not the one that opened the thread", () => {
    const owner = "7";
    const iso = new Date().toISOString();
    window.localStorage.setItem(
      `easyquran.sessions.${owner}`,
      JSON.stringify({
        v: 3,
        threads: [
          {
            id: "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d5b1",
            origin: "index",
            originRef: null,
            scope: "surah:5",
            boundEnd: null,
            surah: 5,
            ayah: 1,
            localPage: null,
            ratio: null,
            sourceId: null,
            device: "phone",
            deviceId: "dev-other-phone",
            deviceName: "Pixel 8",
            startedAt: iso,
            updatedAt: iso,
            endedAt: null,
            endReason: null,
            state: "open",
            identityStamp: null,
            cursorStamp: null,
            lifecycleStamp: null,
            dirty: false,
            rejected: false,
            extras: {},
          },
        ],
        progress: {},
        since: 0,
        after: -1,
        epoch: null,
        lastSeenAt: 0,
      }),
    );
    const { store } = makeStore({ authenticated: true, user: { id: 7 } });
    // The first call hydrates the account; this install's id exists only from then on.
    store.enter(surah(5));
    const here = store.deviceId;
    expect(here).not.toBeNull();
    store.cursor(5, 3, null);
    const moved = store.open.find((t) => t.id === "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d5b1");
    expect(moved).toMatchObject({ surah: 5, ayah: 3, deviceId: here });
  });
});
