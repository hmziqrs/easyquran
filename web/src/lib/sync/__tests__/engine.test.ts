import { createSyncEngine, syncRetryDelayMs, type SyncEngine } from "$lib/sync/engine.svelte";
import { createOutbox, memoryQueueStorage, type Outbox, type QueueStorage } from "$lib/sync/outbox";
import { SyncPausedError, type SyncDomain, type SyncMutation, type SyncRoundResult } from "$lib/sync/types";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/environment", () => ({ browser: true }));

interface FakeDomain {
  readonly domain: SyncDomain<string, string>;
  /** One entry per sync() call, in call order; each holds the batch in FIFO order. */
  readonly batches: SyncMutation[][];
  /** One entry per applyServer() call: the drained-batch argument, in order. */
  readonly drainedBatches: Array<SyncMutation[] | undefined>;
  /** States handed to applyServer, in order. */
  readonly states: string[];
  error: Error | null;
  applyError: Error | null;
}

function fakeDomain(name: string): FakeDomain {
  const batches: SyncMutation[][] = [];
  const drainedBatches: Array<SyncMutation[] | undefined> = [];
  const states: string[] = [];
  const handle: FakeDomain = {
    error: null,
    applyError: null,
    batches,
    drainedBatches,
    states,
    domain: {
      name,
      async sync(mutations: SyncMutation<string>[]): Promise<SyncRoundResult<string>> {
        batches.push(mutations);
        if (handle.error) throw handle.error;
        return { applied: mutations.length, state: `${name}#${mutations.length}` };
      },
      applyServer(state: string, drained?: SyncMutation<string>[]): void {
        states.push(state);
        drainedBatches.push(drained);
        if (handle.applyError) throw handle.applyError;
      },
    },
  };
  return handle;
}

interface TestRig {
  readonly engine: SyncEngine;
  readonly outbox: Outbox;
  setOnline(value: boolean): void;
}

function makeEngine(overrides: { domains?: FakeDomain[]; online?: boolean } = {}): TestRig {
  let connected = overrides.online ?? true;
  const outbox = createOutbox(memoryQueueStorage());
  const engine = createSyncEngine({
    outbox,
    domains: (overrides.domains ?? []).map((fake) => fake.domain),
    online: () => connected,
    flushDelayMs: 2_000,
  });
  return {
    engine,
    outbox,
    setOnline(value: boolean) {
      connected = value;
    },
  };
}

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("syncRetryDelayMs", () => {
  it("doubles per consecutive failure and caps at 60s", () => {
    expect(syncRetryDelayMs(1)).toBe(2_000);
    expect(syncRetryDelayMs(2)).toBe(4_000);
    expect(syncRetryDelayMs(3)).toBe(8_000);
    expect(syncRetryDelayMs(4)).toBe(16_000);
    expect(syncRetryDelayMs(5)).toBe(32_000);
    expect(syncRetryDelayMs(6)).toBe(60_000);
    expect(syncRetryDelayMs(9)).toBe(60_000);
  });
});

describe("SyncEngine.flush", () => {
  it("keeps mutations queued and records the error while offline", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a], online: false });
    const mutation = await rig.engine.enqueue("bookmarks", "m1");
    expect(mutation.id).not.toBe("");
    expect(mutation.seq).toBe(1);

    await rig.engine.flush();

    expect(a.batches).toHaveLength(0);
    expect(rig.engine.phase).toBe("error");
    expect(rig.engine.lastError).toBe("offline");
    expect(rig.engine.pending).toBe(1);

    rig.setOnline(true);
    await rig.engine.flush();
    expect(a.batches).toHaveLength(1);
    expect(a.batches[0]!.map((m) => m.payload)).toEqual(["m1"]);
    expect(rig.engine.pending).toBe(0);
  });

  it("drains FIFO, applies server state, and clears status on success", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.enqueue("bookmarks", "m1");
    await rig.engine.enqueue("bookmarks", "m2");
    await rig.engine.enqueue("bookmarks", "m3");

    await rig.engine.flush();

    expect(a.batches).toHaveLength(1);
    expect(a.batches[0]!.map((m) => m.payload)).toEqual(["m1", "m2", "m3"]);
    expect(a.states).toEqual(["bookmarks#3"]);
    expect(rig.engine.phase).toBe("idle");
    expect(rig.engine.pending).toBe(0);
    expect(rig.engine.lastError).toBeNull();
    expect(rig.engine.lastSyncAt).not.toBeNull();
    expect((await rig.outbox.take("bookmarks", 10)).length).toBe(0);
  });

  it("loops take-sized batches until the domain is drained", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    for (let i = 0; i < 101; i += 1) await rig.engine.enqueue("bookmarks", i);

    await rig.engine.flush();

    expect(a.batches.map((batch) => batch.length)).toEqual([100, 1]);
    const seqs = a.batches.flat().map((m) => m.seq);
    expect(seqs).toEqual(Array.from({ length: 101 }, (_, i) => i + 1));
    expect(rig.engine.pending).toBe(0);
  });

  it("isolates domain failures: healthy domain drains, failing one keeps its queue", async () => {
    const good = fakeDomain("good");
    const bad = fakeDomain("bad");
    bad.error = new Error("boom");
    const rig = makeEngine({ domains: [good, bad] });
    await rig.engine.enqueue("good", "g1");
    await rig.engine.enqueue("bad", "b1");

    await rig.engine.flush();

    expect(good.batches).toHaveLength(1);
    expect(good.states).toHaveLength(1);
    expect(bad.batches).toHaveLength(1);
    expect(rig.engine.phase).toBe("error");
    expect(rig.engine.lastError).toBe("boom");
    expect(rig.engine.pending).toBe(1);
    expect(await rig.outbox.count("bad")).toBe(1);
    expect(await rig.outbox.count("good")).toBe(0);
  });

  it("drains only the requested domain when flush(name) is used", async () => {
    const a = fakeDomain("a");
    const b = fakeDomain("b");
    const rig = makeEngine({ domains: [a, b] });
    await rig.engine.enqueue("a", "x");
    await rig.engine.enqueue("b", "y");

    await rig.engine.flush("a");

    expect(a.batches).toHaveLength(1);
    expect(b.batches).toHaveLength(0);
    expect(rig.engine.pending).toBe(1);
  });

  it("coalesces a flush issued while another is in flight", async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let calls = 0;
    const gated: SyncDomain<string, string> = {
      name: "gated",
      async sync(mutations: SyncMutation<string>[]): Promise<SyncRoundResult<string>> {
        calls += 1;
        await gate;
        return { applied: mutations.length, state: "s" };
      },
      applyServer(): void {},
    };
    const outbox = createOutbox(memoryQueueStorage());
    const engine = createSyncEngine({ outbox, domains: [gated], online: () => true });
    // Seed via the outbox (not engine.enqueue) so no debounce timer is scheduled.
    await outbox.enqueue("gated", "x");
    await engine.hydrate();

    const first = engine.flush();
    const second = engine.flush();
    expect(second).toBe(first);
    release();
    await first;
    await second;

    // Push round + the coalesced flush's pull-only round (new contract: an
    // empty queue still syncs once per flush call).
    expect(calls).toBe(2);
    expect(engine.pending).toBe(0);
  });
});

describe("SyncEngine scheduling", () => {
  it("schedules a debounced flush after enqueue", async () => {
    vi.useFakeTimers();
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.enqueue("bookmarks", "x");

    expect(a.batches).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(2_000);
    expect(a.batches).toHaveLength(1);
    expect(rig.engine.pending).toBe(0);
  });

  it("spaces auto-retries further apart on consecutive failures", async () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    const a = fakeDomain("bookmarks");
    a.error = new Error("boom");
    const rig = makeEngine({ domains: [a] });
    // Seed the outbox directly (not engine.enqueue) so no debounce timer interferes.
    await rig.outbox.enqueue("bookmarks", "x");
    await rig.engine.hydrate();

    await rig.engine.flush();
    expect(a.batches).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(1_999);
    expect(a.batches).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1); // 2s: retry #1 -> failure #2
    expect(a.batches).toHaveLength(2);

    await vi.advanceTimersByTimeAsync(3_999);
    expect(a.batches).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1); // 6s: retry #2 -> failure #3
    expect(a.batches).toHaveLength(3);

    await vi.advanceTimersByTimeAsync(8_000); // 14s: retry #3 -> failure #4
    expect(a.batches).toHaveLength(4);
    expect(rig.engine.phase).toBe("error");
    expect(rig.engine.lastError).toBe("boom");
  });

  it("consecutive offline failures grow the retry delay (no network calls made)", async () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a], online: false });
    await rig.outbox.enqueue("bookmarks", "x");
    await rig.engine.hydrate();

    // Two offline rounds: failure #1 schedules 2s, failure #2 reschedules at
    // the grown 4s cadence (syncRetryDelayMs(2)).
    await rig.engine.flush();
    expect(a.batches).toHaveLength(0);
    await rig.engine.flush();
    expect(a.batches).toHaveLength(0);

    rig.setOnline(true); // flag only: no transition listener ran, failures stay
    await vi.advanceTimersByTimeAsync(3_999);
    expect(a.batches).toHaveLength(0); // a flat 2s cadence would have fired by now
    await vi.advanceTimersByTimeAsync(1); // 4s: the grown retry fires and drains
    expect(a.batches).toHaveLength(1);
    expect(a.batches[0]!.map((m) => m.payload)).toEqual(["x"]);
    expect(rig.engine.pending).toBe(0);
  });

  it("hydrates pending count from the outbox", async () => {
    const rig = makeEngine();
    await rig.outbox.enqueue("bookmarks", "x");
    await rig.outbox.enqueue("bookmarks", "y");
    expect(rig.engine.pending).toBe(0);

    await rig.engine.hydrate();

    expect(rig.engine.pending).toBe(2);
  });

  it("re-syncs pending from the durable outbox after a round (another tab drained it)", async () => {
    const a = fakeDomain("bookmarks");
    const storage = memoryQueueStorage();
    const thisTab = createOutbox(storage);
    const otherTab = createOutbox(storage);
    const engine = createSyncEngine({ outbox: thisTab, domains: [a.domain], online: () => true });
    await thisTab.enqueue("bookmarks", "x");
    await engine.hydrate();
    expect(engine.pending).toBe(1);

    // The other tab clears the shared queue; this engine's local arithmetic
    // still says 1.
    expect(await otherTab.clear("bookmarks")).toBe(1);
    expect(engine.pending).toBe(1);

    await engine.flush();

    // Pull-only round (this tab's take saw an empty queue); pending now
    // reflects the durable truth.
    expect(a.batches).toEqual([[]]);
    expect(engine.pending).toBe(0);
  });
});

describe("SyncEngine drain robustness", () => {
  it("hands applyServer the drained batch after removing it from the queue", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.enqueue("bookmarks", "m1");
    await rig.engine.enqueue("bookmarks", "m2");

    await rig.engine.flush();

    expect(a.states).toEqual(["bookmarks#2"]);
    expect(a.drainedBatches).toHaveLength(1);
    expect(a.drainedBatches[0]!.map((m) => m.payload)).toEqual(["m1", "m2"]);
  });

  it("SyncPausedError skips the domain: no error phase, no retry growth, loop stops", async () => {
    vi.useFakeTimers();
    const paused = fakeDomain("paused");
    paused.error = new SyncPausedError("signed out");
    const healthy = fakeDomain("healthy");
    const rig = makeEngine({ domains: [paused, healthy] });
    await rig.engine.enqueue("paused", "x");
    await rig.engine.enqueue("healthy", "y");

    await rig.engine.flush();

    // sync() was entered once and bailed before any transport work; the queue stays.
    expect(paused.batches).toHaveLength(1);
    expect(rig.engine.pending).toBe(1);
    expect(healthy.batches).toHaveLength(1);
    expect(rig.engine.phase).toBe("idle");
    expect(rig.engine.lastError).toBeNull();

    // The enqueue debounce re-enters sync once more, which pauses again —
    // still no error phase, and the queue is untouched.
    await vi.advanceTimersByTimeAsync(2_000);
    expect(paused.batches).toHaveLength(2);
    expect(rig.engine.phase).toBe("idle");

    // No failure count was recorded, so no backoff retry hammers the paused domain.
    await vi.advanceTimersByTimeAsync(60_000);
    expect(paused.batches).toHaveLength(2);
  });

  it("isolates backoff per domain: a poisoned domain never stretches a healthy one", async () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    const bad = fakeDomain("bad");
    bad.error = new Error("boom");
    const good = fakeDomain("good");
    const rig = makeEngine({ domains: [bad, good] });
    await rig.outbox.enqueue("bad", "x");
    await rig.engine.hydrate();

    await rig.engine.flush(); // bad failure #1 -> retry in 2s; good pull-only round
    await vi.advanceTimersByTimeAsync(2_000); // retry -> bad failure #2 (4s next); good pulls
    await vi.advanceTimersByTimeAsync(4_000); // retry -> bad failure #3 (8s next); good pulls
    expect(bad.batches).toHaveLength(3);
    expect(good.batches).toEqual([[], [], []]); // healthy sibling pulls once per round

    // good's first failure must start at the 2s base cadence, not bad's 8s.
    bad.error = null;
    good.error = new Error("later");
    await rig.outbox.enqueue("good", "y");
    await rig.engine.hydrate();
    await rig.engine.flush(); // bad pushes clean, good fails (#1 -> 2s retry)
    expect(bad.batches).toHaveLength(4);
    expect(good.batches).toHaveLength(4); // 3 pulls + 1 failed push

    await vi.advanceTimersByTimeAsync(1_999);
    expect(good.batches).toHaveLength(4);
    await vi.advanceTimersByTimeAsync(1); // 2s: good's own base retry
    expect(good.batches).toHaveLength(5);
  });

  it("a take() throw fails the round, sets phase error, and schedules a retry", async () => {
    vi.useFakeTimers();
    vi.spyOn(Math, "random").mockReturnValue(0);
    const a = fakeDomain("bookmarks");
    let takeAttempts = 0;
    const failing: QueueStorage = {
      ...memoryQueueStorage(),
      async read() {
        takeAttempts += 1;
        throw new Error("idb gone");
      },
    };
    const outbox = createOutbox(failing);
    const engine = createSyncEngine({ outbox, domains: [a.domain], online: () => true });
    await outbox.enqueue("bookmarks", "x");
    await engine.hydrate();

    await engine.flush();

    expect(takeAttempts).toBeGreaterThanOrEqual(1);
    expect(engine.phase).toBe("error");
    expect(engine.lastError).toBe("idb gone");
    expect(engine.pending).toBe(1);

    await vi.advanceTimersByTimeAsync(2_000); // retry re-attempts the take
    expect(takeAttempts).toBeGreaterThanOrEqual(2);
  });

  it("an applyServer throw keeps the drained batch removed and only records the error", async () => {
    const a = fakeDomain("bookmarks");
    a.applyError = new Error("view exploded");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.enqueue("bookmarks", "m1");

    await rig.engine.flush();

    expect(rig.engine.phase).toBe("error");
    expect(rig.engine.lastError).toBe("view exploded");
    // The server accepted the batch; the queue slot is gone and pending settled.
    expect(await rig.outbox.count("bookmarks")).toBe(0);
    expect(rig.engine.pending).toBe(0);
    expect(rig.engine.lastSyncAt).not.toBeNull();
    expect(a.states).toEqual(["bookmarks#1"]);
  });
});

describe("SyncEngine pull-only rounds", () => {
  it("flushes an empty queue as one pull-only round: sync([]), applyServer, lastSyncAt set", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.hydrate();

    await rig.engine.flush();

    expect(a.batches).toEqual([[]]);
    expect(a.states).toEqual(["bookmarks#0"]);
    expect(a.drainedBatches).toEqual([[]]);
    expect(rig.engine.lastSyncAt).not.toBeNull();
    expect(rig.engine.phase).toBe("idle");
    expect(rig.engine.pending).toBe(0);
  });

  it("never loops on empty: one sync round per flush call", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });

    await rig.engine.flush();
    expect(a.batches).toHaveLength(1);

    await rig.engine.flush();
    expect(a.batches).toHaveLength(2);
  });

  it("the periodic timer pulls while online even with nothing queued", async () => {
    vi.useFakeTimers();
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    const teardown = rig.engine.start();
    await rig.engine.hydrate();

    await vi.advanceTimersByTimeAsync(29_999);
    expect(a.batches).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    expect(a.batches).toEqual([[]]);

    teardown();
  });

  it("enqueue-then-flush pushes exactly once; the push response is the pull (no extra round)", async () => {
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    await rig.engine.enqueue("bookmarks", "m1");

    await rig.engine.flush();

    expect(a.batches.map((batch) => batch.map((m) => m.payload))).toEqual([["m1"]]);
    expect(a.states).toEqual(["bookmarks#1"]);
    expect(rig.engine.pending).toBe(0);
  });
});

describe("SyncEngine coalescing", () => {
  it("picks up a mutation enqueued after take() of an in-flight drain (coalesced pass)", async () => {
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const batches: SyncMutation[][] = [];
    let blockFirst = true;
    const gated: SyncDomain<string, string> = {
      name: "gated",
      async sync(mutations: SyncMutation<string>[]): Promise<SyncRoundResult<string>> {
        batches.push(mutations);
        if (blockFirst) {
          blockFirst = false;
          await gate;
        }
        return { applied: mutations.length, state: "s" };
      },
      applyServer(): void {},
    };
    const outbox = createOutbox(memoryQueueStorage());
    const engine = createSyncEngine({ outbox, domains: [gated], online: () => true });
    await outbox.enqueue("gated", "m1");
    await engine.hydrate();

    const first = engine.flush();
    // Wait until the drain is inside sync([m1]) — past its take().
    await vi.waitFor(() => expect(batches.length).toBe(1));
    // m2 lands after the take of the running pass; its flush coalesces.
    await engine.enqueue("gated", "m2");
    const second = engine.flush();
    expect(second).toBe(first);
    release();
    await first;

    // The coalesced extra pass drains the late mutation in the same round; it
    // then finishes with its own pull-only round (one pull per flush call).
    expect(batches.map((batch) => batch.map((m) => m.payload))).toEqual([["m1"], ["m2"], []]);
    expect(engine.pending).toBe(0);
  });
});

describe("SyncEngine.start", () => {
  it("flushes on a false->true online transition and tears every listener down", async () => {
    vi.useFakeTimers();
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a], online: false });
    await rig.outbox.enqueue("bookmarks", "x");
    await rig.engine.hydrate();

    const docAdd = vi.spyOn(document, "addEventListener");
    const docRemove = vi.spyOn(document, "removeEventListener");
    const winAdd = vi.spyOn(window, "addEventListener");
    const winRemove = vi.spyOn(window, "removeEventListener");

    const teardown = rig.engine.start();
    expect(docAdd).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
    expect(winAdd).toHaveBeenCalledWith("online", expect.any(Function));
    expect(winAdd).toHaveBeenCalledWith("offline", expect.any(Function));

    rig.setOnline(true);
    await vi.advanceTimersByTimeAsync(5_000);
    expect(a.batches).toHaveLength(1);
    expect(rig.engine.pending).toBe(0);

    teardown();
    expect(docRemove).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
    expect(winRemove).toHaveBeenCalledWith("online", expect.any(Function));
    expect(winRemove).toHaveBeenCalledWith("offline", expect.any(Function));

    await rig.outbox.enqueue("bookmarks", "y");
    await rig.engine.hydrate();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(a.batches).toHaveLength(1);
  });

  it("flushes periodically while online with pending work", async () => {
    vi.useFakeTimers();
    const a = fakeDomain("bookmarks");
    const rig = makeEngine({ domains: [a] });
    const teardown = rig.engine.start();

    await rig.outbox.enqueue("bookmarks", "x");
    await rig.engine.hydrate();
    expect(a.batches).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(30_000);
    expect(a.batches).toHaveLength(1);
    expect(rig.engine.pending).toBe(0);

    teardown();
  });
});
