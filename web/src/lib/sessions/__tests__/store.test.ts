import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { RegisteredSyncDomain, SyncMutation } from "#lib/sync/index.js";

import {
  LOCAL_DEFAULT_SESSION_ID,
  accountDefaultSessionId,
  decodeSessionsEnvelope,
  sessionBucketFor,
  type SessionMutation,
  type SessionSnapshot,
} from "../schema.svelte";
import { SessionsStore, type SessionsEngineLike } from "../store.svelte";

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: "https://eq.test/api",
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

/** Owner of the moment for the store's auth-state seam. */
interface FakeAuth {
  authenticated: boolean;
  user: { id: number } | null;
}

interface FakeEngine {
  readonly engine: SessionsEngineLike;
  readonly enqueued: Array<{ domain: string; payload: SessionMutation }>;
  readonly flushed: Array<string | undefined>;
  started: number;
}

function fakeEngine(): FakeEngine {
  const enqueued: Array<{ domain: string; payload: SessionMutation }> = [];
  const flushed: Array<string | undefined> = [];
  const handle: FakeEngine = {
    enqueued,
    flushed,
    started: 0,
    engine: {
      async enqueue<P>(domain: string, payload: P) {
        // SAFETY: test double — the only producer is SessionsStore.#enqueue, whose payloads are SessionMutation by construction.
        enqueued.push({ domain, payload: payload as SessionMutation });
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
    },
  };
  return handle;
}

function makeStore(auth: FakeAuth, engine: FakeEngine) {
  const registered: RegisteredSyncDomain[] = [];
  const unregistered: string[] = [];
  const store = new SessionsStore({
    engine: engine.engine,
    // SAFETY: test double — unsafeRequest is never called in these tests (no live rounds).
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
    unregister: (name) => unregistered.push(name),
  });
  return { store, registered, unregistered };
}

function stateAt(num: number, n: number, furthest: number) {
  return {
    lastRead: { num, n },
    lastReadAnchor: null,
    recents: [],
    progress: { [num]: { furthestAyah: furthest, ts: 1 } },
  };
}

/** Durable mutations mirroring everything `engine` has enqueued (applyServer's `drained`). */
function drainedOf(engine: FakeEngine): SyncMutation<SessionMutation>[] {
  return engine.enqueued.map((entry, index) => ({
    id: `m${index + 1}`,
    domain: entry.domain,
    seq: index + 1,
    payload: entry.payload,
    queuedAt: 0,
  }));
}

function snapshotWithPosition(
  sessionId: string,
  position: { surah: number; ayah: number; updatedAt?: string },
): SessionSnapshot {
  return {
    applied: 0,
    skipped: 0,
    sessions: [
      {
        id: sessionId,
        name: "Reading",
        kind: "free",
        translationId: null,
        createdAt: "2026-01-01T00:00:00.000Z",
        updatedAt: "2026-01-01T00:00:00.000Z",
        archivedAt: null,
      },
    ],
    positions: [
      {
        sessionId,
        surah: position.surah,
        ayah: position.ayah,
        localPage: null,
        ratio: null,
        sourceId: null,
        updatedAt: position.updatedAt ?? "2026-01-01T00:00:00.000Z",
      },
    ],
    progress: [],
  };
}

function storedState(owner: number) {
  return decodeSessionsEnvelope(
    JSON.parse(localStorage.getItem(`easyquran.sessions.${owner}`) ?? "null"),
  );
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SessionsStore owner partitions (§9.5)", () => {
  it("seeds the local default session from the legacy reader blob exactly once", () => {
    localStorage.setItem(
      "easyquran.reader",
      JSON.stringify({ v: 4, current: 1, lastRead: { num: 2, n: 255 }, recents: [], progress: {} }),
    );
    const { store } = makeStore({ authenticated: false, user: null }, fakeEngine());
    expect(store.activeId).toBe(LOCAL_DEFAULT_SESSION_ID);
    const stored = decodeSessionsEnvelope(
      JSON.parse(localStorage.getItem("easyquran.sessions.local") ?? "null"),
    );
    expect(stored?.state[LOCAL_DEFAULT_SESSION_ID]?.lastRead).toEqual({ num: 2, n: 255 });
    // Second boot: the key exists, so the (now changed) blob never re-seeds.
    localStorage.setItem(
      "easyquran.reader",
      JSON.stringify({ v: 4, current: 1, lastRead: { num: 3, n: 1 }, recents: [], progress: {} }),
    );
    const { store: again } = makeStore({ authenticated: false, user: null }, fakeEngine());
    expect(again.list[0]?.name).toBe("Reading");
    const storedAgain = decodeSessionsEnvelope(
      JSON.parse(localStorage.getItem("easyquran.sessions.local") ?? "null"),
    );
    expect(storedAgain?.state[LOCAL_DEFAULT_SESSION_ID]?.lastRead).toEqual({ num: 2, n: 255 });
  });

  it("an account owner gets the deterministic UUIDv5 default (EC-22) and its own key", () => {
    const { store } = makeStore({ authenticated: true, user: { id: 7 } }, fakeEngine());
    expect(store.activeId).toBe(accountDefaultSessionId(7));
    expect(localStorage.getItem("easyquran.sessions.7")).not.toBeNull();
    expect(localStorage.getItem("easyquran.sessions.local")).toBeNull();
  });
});

describe("SessionsStore session switch isolation", () => {
  it("switching never clobbers another session's position or progress", () => {
    const engine = fakeEngine();
    const { store } = makeStore({ authenticated: false, user: null }, engine);
    const applied: Array<{ lastRead: { num: number; n: number } | null }> = [];
    store.onApplyState((state) => applied.push({ lastRead: state.lastRead }));

    store.recordActive(stateAt(2, 100, 100));
    const daily = store.create("Daily");
    expect(daily).not.toBeNull();
    expect(store.activeId).toBe(daily?.id);
    // New session starts empty; the default session keeps 2:100.
    expect(applied.at(-1)?.lastRead).toBeNull();
    expect(store.switchTo(LOCAL_DEFAULT_SESSION_ID)).toBe(true);
    store.recordActive(stateAt(2, 120, 120));
    expect(store.switchTo(daily?.id ?? "")).toBe(true);
    // Daily still has no position; switching back finds 2:120 intact.
    expect(applied.at(-1)?.lastRead).toBeNull();
    expect(store.switchTo(LOCAL_DEFAULT_SESSION_ID)).toBe(true);
    expect(applied.at(-1)?.lastRead).toEqual({ num: 2, n: 120 });
  });

  it("removing the active session falls back to another live session", () => {
    const { store } = makeStore({ authenticated: false, user: null }, fakeEngine());
    const extra = store.create("Study");
    store.remove(LOCAL_DEFAULT_SESSION_ID);
    expect(store.list.map((session) => session.name)).toEqual(["Study"]);
    expect(store.activeId).toBe(extra?.id);
  });
});

describe("SessionsStore sync (§9.2/§9.5)", () => {
  it("recordActive enqueues coalesced position+progress for the account partition only", async () => {
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store } = makeStore({ authenticated: true, user: { id: 7 } }, engine);
    store.onAuthTransition();
    const active = store.activeId;
    expect(active).not.toBeNull();
    store.recordActive(stateAt(2, 255, 255));
    store.recordActive(stateAt(2, 260, 260));
    expect(engine.enqueued.length).toBe(1); // registration's default upsert only; pushes debounced
    await vi.advanceTimersByTimeAsync(5000);
    // The default session row goes first (server drops position/progress for
    // sessions it has never seen), then the coalesced position+progress.
    expect(engine.enqueued.map((entry) => entry.payload.kind)).toEqual([
      "session.upsert",
      "position.upsert",
      "progress.upsert",
    ]);
    for (const entry of engine.enqueued) {
      expect(entry.domain).toBe("sessions.user:7");
      if (entry.payload.kind === "position.upsert") {
        expect(entry.payload.sessionId).toBe(active);
        expect(entry.payload.ayah).toBe(260);
      }
    }
  });

  it("markRead after a late login lands position+progress in the outbox (live order)", async () => {
    // Live order: the store boots anonymous (auth probe still in flight), the
    // reader records positions locally, THEN the probe resolves and the
    // account's domain registers — later markRead writes must still sync.
    vi.useFakeTimers();
    const engine = fakeEngine();
    const auth: FakeAuth = { authenticated: false, user: null };
    const { store } = makeStore(auth, engine);
    store.recordActive(stateAt(2, 10, 10));
    await vi.advanceTimersByTimeAsync(5000);
    expect(engine.enqueued.length).toBe(0); // anonymous: nothing queued

    auth.authenticated = true;
    auth.user = { id: 1 };
    store.onAuthTransition();

    store.recordActive(stateAt(2, 255, 255)); // markRead after login
    await vi.advanceTimersByTimeAsync(5000);
    // Default session row first (FIFO — the server's ownership guard needs it
    // before any position/progress row for the same session id).
    expect(engine.enqueued.map((entry) => entry.payload.kind)).toEqual([
      "session.upsert",
      "position.upsert",
      "progress.upsert",
    ]);
    for (const entry of engine.enqueued) {
      expect(entry.domain).toBe("sessions.user:1");
    }
  });

  it("first login uploads the account default session row", () => {
    const engine = fakeEngine();
    const auth: FakeAuth = { authenticated: false, user: null };
    const { store } = makeStore(auth, engine);
    store.recordActive(stateAt(3, 7, 7)); // anonymous reading first
    auth.authenticated = true;
    auth.user = { id: 1 };
    store.onAuthTransition();
    const upsert = engine.enqueued.find((entry) => entry.payload.kind === "session.upsert");
    expect(upsert?.domain).toBe("sessions.user:1");
    if (upsert?.payload.kind === "session.upsert") {
      expect(upsert.payload.id).toBe(accountDefaultSessionId(1));
      expect(upsert.payload.name).toBe("Reading");
    }
  });

  it("registration re-uploads the default row with its local rename and stamp (LWW-safe)", () => {
    const engine = fakeEngine();
    const { store } = makeStore({ authenticated: true, user: { id: 7 } }, engine);
    store.update(accountDefaultSessionId(7), { name: "Morning" });
    store.onAuthTransition();
    const upsert = engine.enqueued.find((entry) => entry.payload.kind === "session.upsert");
    if (upsert?.payload.kind === "session.upsert") {
      expect(upsert.payload.name).toBe("Morning");
      expect(upsert.payload.updatedAt).not.toBe("1970-01-01T00:00:00.000Z");
    } else {
      expect.unreachable("registration must enqueue the default session upsert");
    }
  });

  it("anonymous reads never enqueue (local owner, no partition)", async () => {
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store } = makeStore({ authenticated: false, user: null }, engine);
    store.recordActive(stateAt(1, 1, 1));
    await vi.advanceTimersByTimeAsync(5000);
    expect(engine.enqueued.length).toBe(0);
  });

  it("session CRUD enqueues to the account partition", () => {
    const engine = fakeEngine();
    const { store } = makeStore({ authenticated: true, user: { id: 7 } }, engine);
    const created = store.create("Ramadan");
    expect(created).not.toBeNull();
    const upsert = engine.enqueued.find((entry) => entry.payload.kind === "session.upsert");
    expect(upsert?.domain).toBe("sessions.user:7");
    if (upsert?.payload.kind === "session.upsert") {
      expect(upsert.payload.name).toBe("Ramadan");
      expect(upsert.payload.sessionKind).toBe("free");
    }
    store.remove(created?.id ?? "");
    expect(engine.enqueued.some((entry) => entry.payload.kind === "session.delete")).toBe(true);
  });

  it("applyServer merges LWW sessions, keeps local-only rows, and never regresses progress (EC-05)", async () => {
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store, registered } = makeStore({ authenticated: true, user: { id: 7 } }, engine);
    store.onAuthTransition();
    const created = store.create("Quran study");
    expect(created).not.toBeNull();
    // The default session holds a HIGHER offline progress than the server row.
    expect(store.switchTo(accountDefaultSessionId(7))).toBe(true);

    const snapshot: SessionSnapshot = {
      applied: 0,
      skipped: 0,
      sessions: [
        {
          id: accountDefaultSessionId(7),
          name: "Reading",
          kind: "free",
          translationId: null,
          createdAt: "2026-01-01T00:00:00.000Z",
          updatedAt: "2026-01-01T00:00:00.000Z",
          archivedAt: null,
        },
      ],
      positions: [
        {
          sessionId: accountDefaultSessionId(7),
          surah: 2,
          ayah: 50,
          localPage: 3,
          ratio: 0.5,
          sourceId: null,
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
      ],
      progress: [
        {
          sessionId: accountDefaultSessionId(7),
          surah: 2,
          furthestAyah: 40,
          updatedAt: "2026-01-01T00:00:00.000Z",
        },
      ],
    };
    // With no local write pending, the server position row applies (pure LWW).
    registered[0]?.applyServer(snapshot, []);
    expect(storedState(7)?.state[accountDefaultSessionId(7)]?.lastRead).toEqual({ num: 2, n: 50 });

    // A recordActive AFTER that snapshot is a fresher local write whose push is
    // still debounced: the same stale server row must not overwrite it
    // (position-push round 2).
    store.recordActive(stateAt(2, 60, 60));
    registered[0]?.applyServer(snapshot, []);
    expect(storedState(7)?.state[accountDefaultSessionId(7)]?.lastRead).toEqual({ num: 2, n: 60 });

    // The push drains and the server answers with the pushed row: the overlay
    // retires and the progress merge runs for real — the local 60 beats the
    // server row 40 (EC-05: MAX, never regresses).
    await vi.advanceTimersByTimeAsync(5000);
    const pushedSnapshot: SessionSnapshot = {
      ...snapshot,
      positions: [
        {
          sessionId: accountDefaultSessionId(7),
          surah: 2,
          ayah: 60,
          localPage: 3,
          ratio: 0.5,
          sourceId: null,
          updatedAt: "2026-01-02T00:00:00.000Z",
        },
      ],
    };
    registered[0]?.applyServer(pushedSnapshot, drainedOf(engine));

    // Local-only session survives the merge (its upsert is still queued).
    expect(store.list.map((session) => session.name)).toContain("Quran study");
    const envelope = decodeSessionsEnvelope(
      JSON.parse(localStorage.getItem("easyquran.sessions.7") ?? "null"),
    );
    expect(envelope?.state[accountDefaultSessionId(7)]?.lastRead).toEqual({ num: 2, n: 60 });
    expect(envelope?.state[accountDefaultSessionId(7)]?.progress[2]?.furthestAyah).toBe(60);
  });

  it("logout quarantines: domain unregistered, account key intact, local owner resumes", () => {
    const engine = fakeEngine();
    const auth: FakeAuth = { authenticated: true, user: { id: 7 } };
    const { store, unregistered } = makeStore(auth, engine);
    store.onAuthTransition();
    expect(unregistered.length).toBe(0);

    auth.authenticated = false;
    auth.user = null;
    store.onAuthTransition();
    expect(unregistered).toEqual(["sessions.user:7"]);
    // Durable account key is never destroyed (EC-14).
    expect(localStorage.getItem("easyquran.sessions.7")).not.toBeNull();
    expect(store.activeId).toBe(LOCAL_DEFAULT_SESSION_ID);
  });
});

describe("session buckets (Feature 4.8, EC-36)", () => {
  it("daily buckets follow the device-local day, weekly the device-local ISO week", () => {
    const monday = new Date(2026, 9, 12, 15, 0); // 2026-10-12, a Monday
    expect(sessionBucketFor("daily", monday)).toBe("2026-10-12");
    expect(sessionBucketFor("weekly", monday)).toBe("2026-W42");
    expect(sessionBucketFor("free", monday)).toBeNull();
    expect(sessionBucketFor("shared", monday)).toBeNull();
  });

  it("a mid-week local midnight crossing rolls the day bucket but not the week bucket", () => {
    const wednesdayNight = new Date(2026, 9, 14, 23, 59);
    const thursdayMorning = new Date(2026, 9, 15, 0, 1);
    expect(sessionBucketFor("daily", wednesdayNight)).not.toBe(
      sessionBucketFor("daily", thursdayMorning),
    );
    expect(sessionBucketFor("weekly", wednesdayNight)).toBe(
      sessionBucketFor("weekly", thursdayMorning),
    );
  });
});

describe("SessionsStore position push vs snapshot pulls (position-push)", () => {
  it("a snapshot pull inside the push debounce window never regresses lastRead; the coalesced push still carries the read verse", async () => {
    // Live repro (round 1): Reading session at server position 4:24; the reader
    // marks 105:3; a pull-only sync round (visibilitychange / 30s periodic) lands
    // inside the 4s push debounce. The stale server row must not overwrite the
    // un-pushed lastRead, and the eventual push must carry 105:3 — not 4:24.
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store, registered } = makeStore({ authenticated: true, user: { id: 1 } }, engine);
    store.onAuthTransition();
    const reading = store.create("Reading");
    expect(reading).not.toBeNull();
    const readingId = reading?.id ?? "";
    const stale = snapshotWithPosition(readingId, { surah: 4, ayah: 24 });
    registered[0]?.applyServer(stale, []);
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 4, n: 24 }); // seed sanity

    const applied: Array<{ lastRead: unknown }> = [];
    store.onApplyState((state) => applied.push({ lastRead: state.lastRead }));

    // markRead(105:3) in the reader: progress grows AND lastRead moves, push debounced.
    store.recordActive({
      lastRead: { num: 105, n: 3 },
      lastReadAnchor: null,
      recents: [],
      progress: { 105: { furthestAyah: 3, ts: 1 }, 4: { furthestAyah: 24, ts: 1 } },
    });

    // Pull-only round inside the window: drained is empty (nothing was queued).
    registered[0]?.applyServer(stale, []);

    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 105, n: 3 });
    expect(applied.at(-1)?.lastRead).toEqual({ num: 105, n: 3 }); // mirror never clobbered

    await vi.advanceTimersByTimeAsync(5000);
    const position = engine.enqueued.find((entry) => entry.payload.kind === "position.upsert");
    if (position?.payload.kind !== "position.upsert") {
      expect.unreachable("the coalesced push must include position.upsert, not just progress");
    }
    expect(position.payload.sessionId).toBe(readingId);
    expect(position.payload.surah).toBe(105);
    expect(position.payload.ayah).toBe(3);
    expect(
      engine.enqueued.some(
        (entry) =>
          entry.payload.kind === "progress.upsert" &&
          entry.payload.surah === 105 &&
          entry.payload.furthestAyah === 3,
      ),
    ).toBe(true);
  });

  it("overlay retires once the drained position matches local, so later server rows apply again", async () => {
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store, registered } = makeStore({ authenticated: true, user: { id: 1 } }, engine);
    store.onAuthTransition();
    const reading = store.create("Reading");
    const readingId = reading?.id ?? "";
    store.recordActive(stateAt(105, 3, 3));
    await vi.advanceTimersByTimeAsync(5000);
    expect(engine.enqueued.some((entry) => entry.payload.kind === "position.upsert")).toBe(true);

    // The batch drains; the server snapshot now agrees with the local position.
    registered[0]?.applyServer(
      snapshotWithPosition(readingId, { surah: 105, ayah: 3 }),
      drainedOf(engine),
    );
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 105, n: 3 });

    // Overlay retired: a newer server row (another device) applies freely.
    registered[0]?.applyServer(
      snapshotWithPosition(readingId, {
        surah: 108,
        ayah: 1,
        updatedAt: "2026-02-01T00:00:00.000Z",
      }),
      [],
    );
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 108, n: 1 });
  });

  it("local write past the drained row keeps the overlay until its own push drains", async () => {
    vi.useFakeTimers();
    const engine = fakeEngine();
    const { store, registered } = makeStore({ authenticated: true, user: { id: 1 } }, engine);
    store.onAuthTransition();
    const reading = store.create("Reading");
    const readingId = reading?.id ?? "";
    store.recordActive(stateAt(105, 3, 3));
    await vi.advanceTimersByTimeAsync(5000); // pos(105:3) queued
    store.recordActive(stateAt(105, 4, 4)); // newer local write, push re-debounced

    // The older batch drains while the newer write is still un-pushed: the stale
    // row must not overwrite 105:4.
    registered[0]?.applyServer(
      snapshotWithPosition(readingId, { surah: 105, ayah: 3 }),
      drainedOf(engine),
    );
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 105, n: 4 });

    await vi.advanceTimersByTimeAsync(5000); // pos(105:4) queued
    registered[0]?.applyServer(
      snapshotWithPosition(readingId, { surah: 105, ayah: 4 }),
      drainedOf(engine),
    );
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 105, n: 4 });
    // Retired for real this time.
    registered[0]?.applyServer(
      snapshotWithPosition(readingId, {
        surah: 108,
        ayah: 2,
        updatedAt: "2026-02-01T00:00:00.000Z",
      }),
      [],
    );
    expect(storedState(1)?.state[readingId]?.lastRead).toEqual({ num: 108, n: 2 });
  });
});
