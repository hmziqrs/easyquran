import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { AuthRequestResult, UnsafeRequestInit } from "#lib/auth/auth-client.js";
import type { JsonRecord } from "#lib/storage/decoders.js";
import type { RegisteredSyncDomain } from "#lib/sync/index.js";
import { SyncPausedError, SyncRetryAfterError } from "#lib/sync/index.js";
import { isUuidV7 } from "#lib/sync/ids.js";

import { noteLeaving } from "../entry";
import type { ReaderTarget } from "../store.svelte";
import { SessionsStore } from "../store.svelte";

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
}

interface FakeResponse {
  readonly status?: number;
  readonly retryAfter?: number;
  readonly body?: Row;
  /** Runs while the request is in the air. */
  readonly during?: () => void;
}

function roundBody(over: Row = {}) {
  return {
    serverTime: Date.now(),
    syncEpoch: "1",
    results: [],
    sessions: [],
    progress: [],
    rev: 0,
    latestRev: 0,
    more: false,
    tombstoneFloor: 0,
    ...over,
  };
}

/** Echoes every sent row as applied, unless a test queues its own answers. */
function harness(userId = 7) {
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
        id: m.kind === "progress.upsert" ? `progress:${JSON.stringify(m.surah)}` : m.id,
        status: "applied",
        stamps: {
          identity: m.identityStamp ?? null,
          cursor: m.cursorStamp ?? null,
          lifecycle: m.lifecycleStamp ?? null,
        },
      })),
    );
    const data = next?.body ?? roundBody({ results, rev: 1, latestRev: 1 });
    const result: AuthRequestResult<never> = {
      ok: status < 400,
      status,
      // SAFETY: test double — the domain decodes this body as unknown.
      data: (status < 400 ? data : null) as never,
      error: null,
      rotated: false,
    };
    return next?.retryAfter === undefined ? result : { ...result, retryAfter: next.retryAfter };
  };
  const registered: RegisteredSyncDomain[] = [];
  const auth = { authenticated: true, user: { id: userId } };
  const store = new SessionsStore({
    engine: { flush: async () => undefined, start: () => () => undefined },
    auth: {
      unsafeRequest: async <T>(_path: string, init: UnsafeRequestInit) => {
        // SAFETY: test double — the domain always sends an object body.
        const body = init.body as Row;
        bodies.push(body);
        // SAFETY: test double — `never` data is assignable to whatever the domain asks for.
        return respond(body) as AuthRequestResult<T>;
      },
      get: async <T>(): Promise<{ ok: boolean; data?: T | null }> => ({ ok: false }),
    },
    state: auth,
    register: (domain) => registered.push(domain),
    unregister: () => undefined,
  });
  store.onAuthTransition();
  const round = async () => {
    const domain = registered.at(-1);
    if (domain?.round === undefined) throw new Error("no row domain");
    return domain.round();
  };
  return { store, bodies, answers, round, auth };
}

/** Plain JSON: undefined fields gone, shape left to the row decoders. */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- test helper over arbitrary fixtures; JSON round-trip is the point.
function plain<T>(value: unknown): T {
  return JSON.parse(JSON.stringify(value));
}

function signedOut(): FakeAuth {
  return { authenticated: false, user: null };
}

const surah = (num: number): ReaderTarget => ({
  scope: `surah:${num}`,
  surah: num,
  ayah: 1,
  exact: false,
  sourceId: null,
});

function read(store: SessionsStore, num: number, ayah: number): string {
  const { thread } = store.enter(surah(num));
  store.cursor(num, ayah, null);
  return thread.id;
}

function storedRow(id: string, owner = "7"): Row | undefined {
  const raw = JSON.parse(localStorage.getItem(`easyquran.sessions.${owner}`) ?? "{}");
  return (raw.threads ?? []).find((t: Row) => t.id === id);
}

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  noteLeaving(false);
});

describe("the row is the queue", () => {
  it("sends dirty rows with stamps, and an ack clears them", async () => {
    const { store, bodies, round } = harness();
    const id = read(store, 2, 30);
    expect(store.dirtyRows().length).toBeGreaterThan(0);
    await round();
    // SAFETY: test double — the store always sends an array of rows.
    const sent = bodies[0]?.mutations as Row[];
    expect(sent.find((m) => m.id === id)).toMatchObject({
      kind: "session.upsert",
      surah: 2,
      ayah: 30,
      cursorStamp: expect.any(String),
      identityStamp: expect.any(String),
      lifecycleStamp: expect.any(String),
    });
    expect(sent.some((m) => m.kind === "progress.upsert" && m.surah === 2)).toBe(true);
    expect(bodies[0]).toMatchObject({ expectedUserId: 7, client: { protocol: 1, platform: "web" } });
    expect(store.dirtyRows()).toEqual([]);
  });

  it("many offline scrolls are one dirty row", () => {
    const { store } = harness();
    const id = read(store, 2, 2);
    for (let ayah = 3; ayah < 200; ayah += 1) store.cursor(2, ayah, null);
    const threads = store.dirtyRows().filter((r) => r.kind === "session.upsert");
    expect(threads).toHaveLength(1);
    expect(threads[0]).toMatchObject({ id, ayah: 199 });
  });

  it("an edit during the request stays dirty", async () => {
    const { store, answers, round, bodies } = harness();
    const id = read(store, 2, 10);
    answers.push({ during: () => store.cursor(2, 11, null) });
    await round();
    expect(bodies).toHaveLength(1);
    const row = store.dirtyRows().find((r) => r.kind === "session.upsert");
    expect(row).toMatchObject({ id, ayah: 11 });
  });

  it("a kill at any step loses nothing: a dirty row is on disk before any request", async () => {
    const first = harness();
    const id = read(first.store, 2, 10);
    first.store.dismiss(id);
    // Lifecycle writes persist at once, with the flag on the row.
    expect(storedRow(id)).toMatchObject({ state: "ended", dirty: true });

    // New process, before any ack: the row is still queued.
    const second = harness();
    expect(second.store.dirtyRows().some((r) => r.kind === "session.upsert" && r.id === id)).toBe(
      true,
    );
  });

  it("a lost response keeps rows dirty, and the replay is harmless", async () => {
    const { store, answers, round, bodies } = harness();
    const id = read(store, 2, 10);
    answers.push("throw");
    await expect(round()).rejects.toThrow("connection lost");
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert" && r.id === id)).toBe(true);
    await round();
    expect(bodies).toHaveLength(2);
    expect(store.dirtyRows()).toEqual([]);
  });

  it("session expiry keeps dirty rows for that account", async () => {
    const { store, answers, round } = harness();
    const id = read(store, 2, 10);
    answers.push({ status: 401 });
    await expect(round()).rejects.toBeInstanceOf(SyncPausedError);
    store.dismiss(id);
    expect(storedRow(id)).toMatchObject({ dirty: true });
    const again = harness();
    expect(again.store.dirtyRows().length).toBeGreaterThan(0);
  });

  it("a full local store never reports rows as synced", () => {
    const { store } = harness();
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => {
        throw new DOMException("full", "QuotaExceededError");
      },
      removeItem: () => undefined,
    });
    try {
      const id = read(store, 2, 10);
      store.dismiss(id);
      expect(store.storageFailed).toBe(true);
      expect(store.dirtyRows().some((r) => r.kind === "session.upsert" && r.id === id)).toBe(true);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("a dirty row survives the local cap", async () => {
    const { store } = harness();
    const ids: string[] = [];
    for (let i = 1; i <= 80; i += 1) {
      const id = read(store, (i % 100) + 1, 2);
      store.dismiss(id);
      ids.push(id);
    }
    const sent = store.dirtyRows();
    expect(sent.filter((r) => r.kind === "session.upsert")).toHaveLength(80);
  });
});

describe("per-row results", () => {
  it("rejected keeps the row, stops resending, and an edit revives it", async () => {
    const { store, answers, round } = harness();
    const id = read(store, 2, 10);
    answers.push({ body: roundBody({ results: [{ id, status: "rejected", reason: "bad" }] }) });
    await round();
    expect(store.rejectedCount).toBe(1);
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert")).toBe(false);
    store.cursor(2, 11, null);
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert")).toBe(true);
  });

  it("capped stays dirty, is surfaced, and retries", async () => {
    const { store, answers, round } = harness();
    const id = read(store, 2, 10);
    answers.push({ body: roundBody({ results: [{ id, status: "capped" }] }) });
    await round();
    expect(store.cappedCount).toBe(1);
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert" && r.id === id)).toBe(true);
    await round();
    expect(store.cappedCount).toBe(0);
  });

  it("adopts a stamp the server rewrote", async () => {
    const { store, answers, round } = harness();
    const id = read(store, 2, 10);
    const lower = `${Date.now() - 5000}-0000-dev`;
    answers.push({
      body: roundBody({
        results: [{ id, status: "applied", stamps: { identity: lower, cursor: lower, lifecycle: lower } }],
      }),
    });
    await round();
    expect(store.open[0]?.cursorStamp).toBe(lower);
    expect(store.dirtyRows().filter((r) => r.kind === "session.upsert")).toEqual([]);
  });

  it("maps every failure status", async () => {
    const { answers, round, store } = harness();
    answers.push({ status: 403 });
    await expect(round()).rejects.toBeInstanceOf(SyncPausedError);
    answers.push({ status: 429, retryAfter: 90 });
    await expect(round()).rejects.toMatchObject({ seconds: 90 });
    answers.push({ status: 429 });
    await expect(round()).rejects.toBeInstanceOf(SyncRetryAfterError);
    answers.push({ status: 503 });
    await expect(round()).rejects.toThrow("503");
    answers.push({ status: 404 });
    await expect(round()).rejects.toThrow("404");
    answers.push({ status: 426 });
    await expect(round()).rejects.toBeInstanceOf(SyncPausedError);
    expect(store.updateRequired).toBe(true);
  });
});

describe("delta pull", () => {
  const remote = (id: string, over: Row = {}) => ({
    id,
    origin: "index",
    originRef: null,
    scope: "surah:9",
    boundEnd: null,
    surah: 9,
    ayah: 4,
    localPage: null,
    ratio: null,
    sourceId: null,
    device: "phone",
    deviceId: "other",
    startedAt: "2026-11-01T00:00:00.000Z",
    updatedAt: "2026-11-01T00:00:00.000Z",
    endedAt: null,
    endReason: null,
    state: "open",
    identityStamp: `${Date.now() - 1000}-0000-other`,
    cursorStamp: `${Date.now() - 1000}-0000-other`,
    lifecycleStamp: `${Date.now() - 1000}-0000-other`,
    rev: 5,
    ...over,
  });
  const RID = "019a0000-0000-7000-8000-000000000009";

  it("an empty response deletes nothing", async () => {
    const { store, round } = harness();
    read(store, 2, 10);
    await round();
    expect(store.open).toHaveLength(1);
    await round();
    expect(store.open).toHaveLength(1);
  });

  it("deletes only on a tombstone, and saves since after the page lands", async () => {
    const { store, answers, round, bodies } = harness();
    answers.push({ body: roundBody({ sessions: [remote(RID)], rev: 5, latestRev: 5 }) });
    await round();
    expect(store.open.map((t) => t.id)).toEqual([RID]);
    answers.push({
      body: roundBody({
        sessions: [
          {
            id: RID,
            state: "deleted",
            rev: 6,
            identityStamp: "1",
            cursorStamp: "1",
            lifecycleStamp: `${Date.now()}-0000-other`,
          },
        ],
        rev: 6,
        latestRev: 6,
      }),
    });
    await round();
    expect(store.open).toEqual([]);
    await round();
    expect(bodies.at(-1)).toMatchObject({ since: 6 });
  });

  it("a page that never landed is pulled again from the same place", async () => {
    const { answers, round, bodies } = harness();
    answers.push({ body: roundBody({ rev: 100, latestRev: 500, more: true }) });
    await round();
    expect(bodies.at(-1)).toMatchObject({ since: 0, after: -1 });
    answers.push("throw");
    await expect(round()).rejects.toThrow();
    expect(bodies.at(-1)).toMatchObject({ since: 0, after: 100 });
    answers.push({ body: roundBody({ rev: 500, latestRev: 500 }) });
    await round();
    await round();
    expect(bodies.at(-2)).toMatchObject({ since: 0, after: 100 });
  });

  it("past the floor: full pull, and what the server no longer holds goes", async () => {
    const { store, answers, round, bodies } = harness();
    const id = read(store, 2, 10);
    answers.push({
      body: roundBody({
        results: plain<Row[]>(
          store.dirtyRows().map((r) => ({
            id: r.kind === "session.upsert" ? r.id : `progress:${r.surah}`,
            status: "applied",
            stamps:
              r.kind === "session.upsert"
                ? { identity: r.identityStamp, cursor: r.cursorStamp, lifecycle: r.lifecycleStamp }
                : null,
          })),
        ),
        rev: 10,
        latestRev: 10,
      }),
    });
    await round();
    expect(store.open.map((t) => t.id)).toEqual([id]);
    answers.push({ body: roundBody({ rev: 10, latestRev: 10 }) });
    const old = await round();
    expect(old.more).toBe(false);

    answers.push({ body: roundBody({ rev: 50, latestRev: 50, tombstoneFloor: 40 }) });
    const reset = await round();
    expect(reset.more).toBe(true);
    answers.push({ body: roundBody({ rev: 50, latestRev: 50, tombstoneFloor: 40 }) });
    await round();
    expect(bodies.at(-1)).toMatchObject({ since: 0 });
    expect(store.open).toEqual([]);
  });

  it("a restored server forces a full pull and re-push", async () => {
    const { store, answers, round, bodies } = harness();
    read(store, 2, 10);
    await round();
    expect(store.dirtyRows()).toEqual([]);
    answers.push({ body: roundBody({ syncEpoch: "2", rev: 1, latestRev: 1 }) });
    const res = await round();
    expect(res.more).toBe(true);
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert")).toBe(true);
    await round();
    expect(bodies.at(-1)).toMatchObject({ since: 0 });
  });

  it("an unknown enum shows a neutral default and a later round keeps working", async () => {
    const { store, answers, round } = harness();
    answers.push({
      body: roundBody({
        sessions: [remote(RID, { origin: "watch-face", device: "watch", extraField: 1 })],
        rev: 5,
        latestRev: 5,
      }),
    });
    await round();
    expect(store.open[0]).toMatchObject({ origin: "link", device: "desktop" });
    expect(store.open[0]?.extras).toEqual({ extraField: 1 });
  });

  it("a merge keeps ended threads ended when another device's cursor lands", async () => {
    const { store, answers, round } = harness();
    const id = read(store, 2, 10);
    store.dismiss(id);
    await round();
    const later = `${Date.now() + 5000}-0000-other`;
    const row = remote(id, { surah: 2, ayah: 99, cursorStamp: later, state: "open", lifecycleStamp: "0000000000001-0000-x" });
    answers.push({ body: roundBody({ sessions: [row], rev: 7, latestRev: 7 }) });
    await round();
    expect(store.open).toEqual([]);
  });
});

describe("accounts", () => {
  it("signing in folds the device's threads in with their own time", async () => {
    const auth = signedOut();
    const registered: RegisteredSyncDomain[] = [];
    const store = new SessionsStore({
      engine: { flush: async () => undefined, start: () => () => undefined },
      auth: {
        unsafeRequest: async () => ({ ok: false, status: 500, data: null, error: null, rotated: false }),
        get: async <T>(): Promise<{ ok: boolean; data?: T | null }> => ({ ok: false }),
      },
      state: auth,
      register: (domain) => registered.push(domain),
      unregister: () => undefined,
    });
    store.enter(surah(18));
    store.cursor(18, 10, null);
    store.dismissOffer();
    const local = store.open[0];
    expect(local?.cursorStamp).toBeNull();
    auth.authenticated = true;
    auth.user = { id: 9 };
    store.onAuthTransition();
    expect(localStorage.getItem("easyquran.sessions.local")).toBeNull();
    const rows = store.dirtyRows();
    expect(rows.find((r) => r.kind === "session.upsert")).toMatchObject({
      surah: 18,
      ayah: 10,
      cursorStamp: expect.stringMatching(/^\d{13}-0000-/u),
    });
    expect(registered.map((d) => d.name)).toEqual(["sessions.user:9"]);
  });

  it("two accounts on one device share no rows or device id", () => {
    const first = harness(7);
    read(first.store, 2, 5);
    first.store.dismiss(first.store.open[0]?.id ?? "");
    const second = harness(9);
    expect(second.store.dirtyRows()).toEqual([]);
    expect(localStorage.getItem("easyquran.device.id.7")).not.toBe(
      localStorage.getItem("easyquran.device.id.9"),
    );
  });

  it("a store newer than this build is left alone and sync pauses", () => {
    const newer = JSON.stringify({ v: 99, threads: [], progress: {} });
    localStorage.setItem("easyquran.sessions.7", newer);
    const { store } = harness();
    read(store, 2, 5);
    expect(localStorage.getItem("easyquran.sessions.7")).toBe(newer);
  });
});

describe("legacy thread ids", () => {
  const LEGACY = "c1c1c1c1-c1c1-4c1c-8c1c-c1c1c1c1c1c1";

  it("re-keys a stored v4 thread to a v7 built from startedAt, and syncs it", () => {
    const startedAt = "2026-10-01T00:00:00.000Z";
    localStorage.setItem(
      "easyquran.sessions.7",
      JSON.stringify({
        v: 2,
        threads: [
          {
            id: LEGACY,
            origin: "index",
            scope: "surah:2",
            surah: 2,
            ayah: 5,
            device: "phone",
            startedAt,
            updatedAt: startedAt,
          },
        ],
        progress: {},
      }),
    );
    const { store } = harness();
    const row = store.dirtyRows().find((r) => r.kind === "session.upsert");
    const id = row?.kind === "session.upsert" ? row.id : "";
    expect(isUuidV7(id)).toBe(true);
    expect(id.replaceAll("-", "").slice(0, 12)).toBe(
      Date.parse(startedAt).toString(16).padStart(12, "0"),
    );
    expect(store.dirtyRows().some((r) => r.kind === "session.upsert" && r.id === LEGACY)).toBe(false);
  });
});
