import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$service-worker", () => ({
  base: "",
  // SAFETY: $service-worker mock; build is typed string[] but stays empty because no test enumerates built assets.
  build: [] as string[],
  // SAFETY: $service-worker mock; files is typed string[] but stays empty because no test enumerates static files.
  files: [] as string[],
  version: "test-v1",
}));

const { memIdb } = vi.hoisted(() => ({ memIdb: new Map<string, Map<string, unknown>>() }));

vi.mock("../../../../lib/workers/idb", () => ({
  IDB_VERSION: 1,
  openIdb: async (db: string, store: string) => ({ db, store }),
  idbGet: async (h: { db: string; store: string }, store: string, key: string) =>
    memIdb.get(`${h.db} ${store}`)?.get(key),
  idbPut: async (
    h: { db: string; store: string },
    store: string,
    // eslint-disable-next-line anti-slop/no-unknown-parameters -- mock mirrors the real idbPut opaque value boundary; values are heterogeneous service-worker metadata never inspected by this mock.
    value: unknown,
    key?: string,
  ) => {
    const k = `${h.db} ${store}`;
    if (!memIdb.has(k)) memIdb.set(k, new Map());
    if (key !== undefined) memIdb.get(k)!.set(key, value);
  },
  idbDelete: async (h: { db: string; store: string }, store: string, key: string) => {
    memIdb.get(`${h.db} ${store}`)?.delete(key);
  },
  idbScan: async (h: { db: string; store: string }, store: string, prefix: string) => {
    const storeMap = memIdb.get(`${h.db} ${store}`);
    if (!storeMap) return {};
    const entries: Array<[string, unknown]> = [];
    for (const [k, v] of storeMap) {
      if (k.startsWith(prefix)) entries.push([k.slice(prefix.length), v]);
    }
    return Object.fromEntries(entries);
  },
  runTxVoid: async () => {},
}));

import { VERSION_QUERY, VERSION_RESULT } from "../../../../lib/offline/messages";
import {
  PRUNE_DEADLINE_KEY,
  PRUNE_GRACE_MS,
  maybeFinalizeHandoff,
  maybePruneAfterGrace,
  purgeAllDataMeta,
  versionQueryHandler,
} from "../../../../service-worker";

const ORIGIN = "https://easyquran.fyi";
const SW_META_MAP_KEY = "easyquran-sw-meta meta";
// Frozen epoch all deadline arithmetic is measured against.
const T0 = Date.parse("2026-01-01T00:00:00.000Z");

function metaMap(): Map<string, unknown> {
  let m = memIdb.get(SW_META_MAP_KEY);
  if (!m) {
    m = new Map();
    memIdb.set(SW_META_MAP_KEY, m);
  }
  return m;
}

function setAck(clientId: string, ackedVersion: string): void {
  metaMap().set(`ack:${clientId}`, ackedVersion);
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- stubs the WorkerGlobalScope clients holder; the SUT reads it via the real Clients type at call time, not this local annotation.
function setClients(ids: string[]): void {
  const live = ids.map((id) => ({ id }));
  // SAFETY: the SW module captured `self` (happy-dom window) at import; assigning a one-method Clients double in place is the only seam, and the SUT consumes just matchAll -> Array<{id}>.
  // eslint-disable-next-line anti-slop/no-chained-type-assertions, anti-slop/require-safety-comment-for-type-assertion -- the DOM Clients interface cannot be replicated by a one-method fake; collapse-to-one assertion does not compile
  (self as unknown as { clients: unknown }).clients = {
    matchAll: async (): Promise<typeof live> => live,
  };
}

class FakeCache {
  readonly entries = new Map<string, Response>();

  async match(req: Request | string): Promise<Response | undefined> {
    const key = req instanceof Request ? req.url : req;
    return this.entries.get(key);
  }

  async put(req: Request | string, res: Response): Promise<void> {
    const key = req instanceof Request ? req.url : req;
    this.entries.set(key, res);
  }

  async delete(req: Request | string): Promise<boolean> {
    const key = req instanceof Request ? req.url : req;
    return this.entries.delete(key);
  }

  async keys(): Promise<Request[]> {
    return [...this.entries.keys()].map((k) => new Request(k));
  }
}

class FakeCacheStorage {
  readonly caches = new Map<string, FakeCache>();

  async open(name: string): Promise<FakeCache> {
    let c = this.caches.get(name);
    if (!c) {
      c = new FakeCache();
      this.caches.set(name, c);
    }
    return c;
  }

  async has(name: string): Promise<boolean> {
    return this.caches.has(name);
  }

  async delete(name: string): Promise<boolean> {
    return this.caches.delete(name);
  }

  async keys(): Promise<string[]> {
    return [...this.caches.keys()];
  }
}

interface FakePort {
  postMessage(msg: { type: string }): void;
  close(): void;
  onmessage: ((ev: { data: unknown }) => void) | null;
  start(): void;
}

function fakePort() {
  const messages: unknown[] = [];
  const port: FakePort = {
    postMessage: (msg: { type: string }) => {
      messages.push(msg);
    },
    close: () => {},
    onmessage: null,
    start: () => {},
  };
  // SAFETY: FakePort implements the MessagePort surface this SUT exercises (postMessage/close/onmessage/start); widened through unknown because MessagePort is a DOM interface with ~15 members/overloads the fake deliberately omits, so no single assertion compiles.
  // eslint-disable-next-line anti-slop/no-chained-type-assertions -- MessagePort's full DOM member/overload set cannot be structurally replicated by the fake double; collapse-to-one assertion does not compile, and widening versionQueryHandler's signature is out of this file's scope.
  return { port: port as unknown as MessagePort, messages };
}

function successResponse(size = 64): Response {
  return new Response(".".repeat(size), {
    headers: { "content-type": "application/json", "content-length": String(size) },
  });
}

function flush(ms = 10): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function seedAppCaches(fakeCaches: FakeCacheStorage): Promise<void> {
  const oldApp = await fakeCaches.open("eq-app-old-v9");
  await oldApp.put(new Request(`${ORIGIN}/_app/immutable/old-chunk.js`), successResponse(10));
  const currentApp = await fakeCaches.open("eq-app-test-v1");
  await currentApp.put(new Request(`${ORIGIN}/_app/immutable/new-chunk.js`), successResponse(10));
  const pack = await fakeCaches.open("eq-pack-alpha");
  await pack.put(new Request(`${ORIGIN}/app/juz/1`), successResponse(10));
}

let fakeCaches: FakeCacheStorage;

beforeEach(async () => {
  memIdb.clear();
  fakeCaches = new FakeCacheStorage();
  vi.stubGlobal("caches", fakeCaches);
  vi.stubGlobal(
    "BroadcastChannel",
    class {
      postMessage(): void {}
      close(): void {}
      addEventListener(): void {}
      removeEventListener(): void {}
    },
  );
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(T0);
  await purgeAllDataMeta();
});

afterEach(async () => {
  vi.useRealTimers();
  await purgeAllDataMeta();
  vi.unstubAllGlobals();
});

describe("VERSION_QUERY handshake", () => {
  it("replies with VERSION_RESULT carrying the worker's own version on the port", () => {
    const { port, messages } = fakePort();
    versionQueryHandler(port);
    expect(messages).toHaveLength(1);
    // SAFETY: messages has length 1 (asserted above); messages[0] is the VERSION_RESULT the handler posted, which always carries type and version.
    expect(messages[0]).toEqual({ type: VERSION_RESULT, version: "test-v1" });
  });

  it("no-ops without throwing when no port is transferred", () => {
    expect(() => versionQueryHandler()).not.toThrow();
  });

  it("dispatching VERSION_QUERY on the worker scope answers on the transferred port", async () => {
    const { port, messages } = fakePort();
    self.dispatchEvent(
      new MessageEvent("message", { data: { type: VERSION_QUERY }, ports: [port] }),
    );
    await flush(10);
    expect(messages).toHaveLength(1);
    // SAFETY: messages has length 1 (asserted above); messages[0] is the VERSION_RESULT the handler posted, which always carries type and version.
    expect(messages[0]).toEqual({ type: VERSION_RESULT, version: "test-v1" });
  });
});

describe("I7 grace prune (persisted deadline, event-driven)", () => {
  it("all-ack persists the deadline but does NOT prune immediately", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    setAck("c1", "test-v1");

    await maybeFinalizeHandoff();

    expect(metaMap().get(PRUNE_DEADLINE_KEY)).toBe(T0 + PRUNE_GRACE_MS);
    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(true);
    expect(fakeCaches.caches.has("eq-app-test-v1")).toBe(true);
    expect(fakeCaches.caches.has("eq-pack-alpha")).toBe(true);
  });

  it("wake after the deadline with acks intact prunes old eq-app-* and clears the deadline", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    setAck("c1", "test-v1");
    await maybeFinalizeHandoff();

    vi.setSystemTime(T0 + PRUNE_GRACE_MS + 1000);
    await maybePruneAfterGrace();

    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(false);
    expect(fakeCaches.caches.has("eq-app-test-v1")).toBe(true);
    expect(fakeCaches.caches.has("eq-pack-alpha")).toBe(true);
    expect(metaMap().has(PRUNE_DEADLINE_KEY)).toBe(false);
  });

  it("wake after the deadline with a stale ack (inherited from a prior worker) does NOT prune", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    // Ack written by the previous worker carries the OLD version string.
    setAck("c1", "old-v9");
    metaMap().set(PRUNE_DEADLINE_KEY, T0 - 1000);

    await maybePruneAfterGrace();

    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(true);
    expect(metaMap().get(PRUNE_DEADLINE_KEY)).toBe(T0 - 1000);
  });

  it("wake after the deadline with a missing ack does NOT prune", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    metaMap().set(PRUNE_DEADLINE_KEY, T0 - 1000);

    await maybePruneAfterGrace();

    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(true);
    expect(metaMap().has(PRUNE_DEADLINE_KEY)).toBe(true);
  });

  it("a new all-ack overwrites (resets) an existing deadline", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    setAck("c1", "test-v1");
    await maybeFinalizeHandoff();
    const firstDeadline = metaMap().get(PRUNE_DEADLINE_KEY);
    expect(firstDeadline).toBe(T0 + PRUNE_GRACE_MS);

    vi.setSystemTime(T0 + 5 * 60 * 1000);
    await maybeFinalizeHandoff();

    expect(metaMap().get(PRUNE_DEADLINE_KEY)).toBe(T0 + 5 * 60 * 1000 + PRUNE_GRACE_MS);
    expect(metaMap().get(PRUNE_DEADLINE_KEY)).not.toBe(firstDeadline);
  });

  it("before the deadline the wake check is a no-op even with acks intact", async () => {
    await seedAppCaches(fakeCaches);
    setClients(["c1"]);
    setAck("c1", "test-v1");
    await maybeFinalizeHandoff();

    vi.setSystemTime(T0 + PRUNE_GRACE_MS - 1000);
    await maybePruneAfterGrace();

    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(true);
    expect(metaMap().has(PRUNE_DEADLINE_KEY)).toBe(true);
  });

  it("no live clients means no deadline is armed and nothing is pruned", async () => {
    await seedAppCaches(fakeCaches);
    setClients([]);

    await maybeFinalizeHandoff();

    expect(metaMap().has(PRUNE_DEADLINE_KEY)).toBe(false);
    expect(fakeCaches.caches.has("eq-app-old-v9")).toBe(true);
  });
});
