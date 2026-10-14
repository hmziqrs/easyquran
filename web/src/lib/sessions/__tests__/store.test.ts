import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { RegisteredSyncDomain, SyncMutation } from "#lib/sync/index.js";

import { markEntry, noteLeaving } from "../entry";
import {
  STALE_AFTER_MS,
  decodeEnvelope,
  type ReadingThread,
  type SessionMutation,
  type SessionSnapshot,
} from "../schema";
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
  const enqueued: Array<{ domain: string; payload: SessionMutation }> = [];
  const engine: SessionsEngineLike = {
    async enqueue<P>(domain: string, payload: P) {
      // SAFETY: test double — SessionsStore only enqueues SessionMutation payloads.
      enqueued.push({ domain, payload: payload as SessionMutation });
      // SAFETY: test double — the store never reads the returned mutation.
      return { id: `m${enqueued.length}`, seq: enqueued.length } as SyncMutation<P>;
    },
    async flush() {},
    start: () => () => undefined,
  };
  return { engine, enqueued };
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
});

describe("account sync", () => {
  const account = (): FakeAuth => ({ authenticated: true, user: { id: 7 } });

  it("pushes thread rows and progress through the account partition", () => {
    vi.useFakeTimers();
    const { store, engine, registered } = makeStore(account());
    store.onAuthTransition();
    expect(registered.map((d) => d.name)).toEqual(["sessions.user:7"]);
    store.enter(surah(2));
    store.cursor(2, 30, null);
    store.cursor(2, 31, null);
    vi.advanceTimersByTime(4100);
    const kinds = engine.enqueued.map((e) => `${e.domain} ${e.payload.kind}`);
    expect(kinds).toEqual(["sessions.user:7 session.upsert", "sessions.user:7 progress.upsert"]);
    expect(engine.enqueued[0]?.payload).toMatchObject({
      surah: 2,
      ayah: 31,
      device: expect.any(String),
    });
  });

  it("applies newer server rows, keeps pending ones, and drops threads ended elsewhere", () => {
    vi.useFakeTimers();
    const { store } = makeStore(account());
    store.onAuthTransition();
    const mine = store.enter(surah(2)).thread;
    store.cursor(2, 10, null);
    vi.advanceTimersByTime(4100);
    markEntry({ origin: "index" });
    const other = store.enter(surah(5)).thread;
    store.cursor(5, 2, null);
    vi.advanceTimersByTime(4100);
    // Pretend both drained; then the server reports only `mine`, moved on another device.
    const drained: SyncMutation<SessionMutation>[] = [mine, other].map((t, i) => ({
      id: `m${i}`,
      domain: "sessions.user:7",
      seq: i,
      queuedAt: 0,
      payload: { kind: "session.upsert", ...rowOf(store, t.id) },
    }));
    const moved: ReadingThread = {
      ...rowOf(store, mine.id),
      ayah: 50,
      device: "phone",
      updatedAt: new Date(Date.now() + 60_000).toISOString(),
    };
    const snapshot: SessionSnapshot = { applied: 0, skipped: 0, sessions: [moved], progress: [] };
    // `other` is this tab's thread: kept even though the server omits it.
    store.applyServer(snapshot, drained);
    expect(store.open.find((t) => t.id === mine.id)).toMatchObject({ ayah: 50, device: "phone" });
    expect(store.open.some((t) => t.id === other.id)).toBe(true);
  });

  it("signing in folds the device's threads into the account silently", () => {
    vi.useFakeTimers();
    const auth: FakeAuth = anon();
    const { store, engine } = makeStore(auth);
    store.enter(surah(18));
    store.cursor(18, 10, null);
    vi.advanceTimersByTime(500);
    expect(window.localStorage.getItem("easyquran.sessions.local")).not.toBeNull();
    auth.authenticated = true;
    auth.user = { id: 9 };
    store.onAuthTransition();
    expect(window.localStorage.getItem("easyquran.sessions.local")).toBeNull();
    expect(store.open[0]).toMatchObject({ surah: 18, ayah: 10 });
    vi.advanceTimersByTime(4100);
    expect(engine.enqueued.map((e) => e.payload.kind)).toContain("session.upsert");
    expect(engine.enqueued.every((e) => e.domain === "sessions.user:9")).toBe(true);
  });
});

function rowOf(store: SessionsStore, id: string): ReadingThread {
  const row = store.open.find((t) => t.id === id);
  if (row === undefined) throw new Error(`thread ${id} not open`);
  return row;
}
