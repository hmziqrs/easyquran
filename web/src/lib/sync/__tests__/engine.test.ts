import { createSyncEngine, syncRetryDelayMs, type SyncEngine } from "$lib/sync/engine.svelte";
import { createOutbox, memoryQueueStorage, type Outbox } from "$lib/sync/outbox";
import type { SyncDomain, SyncMutation, SyncRoundResult } from "$lib/sync/types";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/environment", () => ({ browser: true }));

interface FakeDomain {
  readonly domain: SyncDomain<string, string>;
  /** One entry per sync() call, in call order; each holds the batch in FIFO order. */
  readonly batches: SyncMutation[][];
  /** States handed to applyServer, in order. */
  readonly states: string[];
  error: Error | null;
}

function fakeDomain(name: string): FakeDomain {
  const batches: SyncMutation[][] = [];
  const states: string[] = [];
  const handle: FakeDomain = {
    error: null,
    batches,
    states,
    domain: {
      name,
      async sync(mutations: SyncMutation<string>[]): Promise<SyncRoundResult<string>> {
        batches.push(mutations);
        if (handle.error) throw handle.error;
        return { applied: mutations.length, state: `${name}#${mutations.length}` };
      },
      applyServer(state: string): void {
        states.push(state);
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

    expect(calls).toBe(1);
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

  it("hydrates pending count from the outbox", async () => {
    const rig = makeEngine();
    await rig.outbox.enqueue("bookmarks", "x");
    await rig.outbox.enqueue("bookmarks", "y");
    expect(rig.engine.pending).toBe(0);

    await rig.engine.hydrate();

    expect(rig.engine.pending).toBe(2);
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
