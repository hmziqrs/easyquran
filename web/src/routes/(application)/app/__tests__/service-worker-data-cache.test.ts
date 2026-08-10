import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$service-worker", () => ({
  base: "",
  build: [] as string[],
  files: [] as string[],
  version: "test-v1",
}));

// happy-dom ships no persistent IndexedDB, so route every meta read/write
// through an in-memory store by mocking the idb helper module.
const { memIdb } = vi.hoisted(() => ({ memIdb: new Map<string, Map<string, unknown>>() }));

vi.mock("../../../../lib/workers/idb", () => ({
  IDB_VERSION: 1,
  openIdb: async (db: string, store: string) => ({ db, store }),
  idbGet: async (h: { db: string; store: string }, store: string, key: unknown) =>
    memIdb.get(`${h.db} ${store}`)?.get(key as string),
  idbPut: async (
    h: { db: string; store: string },
    store: string,
    value: unknown,
    key?: unknown,
  ) => {
    const k = `${h.db} ${store}`;
    if (!memIdb.has(k)) memIdb.set(k, new Map());
    if (key !== undefined) memIdb.get(k)!.set(key as string, value);
  },
  idbDelete: async (h: { db: string; store: string }, store: string, key: unknown) => {
    memIdb.get(`${h.db} ${store}`)?.delete(key as string);
  },
  idbScan: async (h: { db: string; store: string }, store: string, prefix: string) => {
    const out: Record<string, unknown> = {};
    const storeMap = memIdb.get(`${h.db} ${store}`);
    if (storeMap) {
      for (const [k, v] of storeMap) {
        if (typeof k === "string" && k.startsWith(prefix)) out[k.slice(prefix.length)] = v;
      }
    }
    return out;
  },
  runTxVoid: async () => {},
}));

import {
  DATA_BUDGET_BYTES,
  DATA_CACHE,
  DATA_MAX,
  deleteDataMeta,
  enforceDataBounds,
  enforceDataBoundsInner,
  handleData,
  isCacheable,
  normalizeDataKey,
  purgeAllDataMeta,
  readDataMeta,
  recordDataEntry,
  scanDataMeta,
  touchDataMeta,
} from "../../../../service-worker";

class FakeCache {
  readonly entries = new Map<string, Response>();

  async match(req: Request | string): Promise<Response | undefined> {
    const key = typeof req === "string" ? req : normalizeDataKey(req.url);
    return this.entries.get(key);
  }

  async put(req: Request | string, res: Response): Promise<void> {
    const key = typeof req === "string" ? req : normalizeDataKey(req.url);
    this.entries.set(key, res);
  }

  async delete(req: Request | string): Promise<boolean> {
    const key = typeof req === "string" ? req : normalizeDataKey(req.url);
    return this.entries.delete(key);
  }

  async keys(): Promise<Request[]> {
    return [...this.entries.keys()].map((k) => new Request(`https://easyquran.fyi${k}`));
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

  async match(req: Request, opts?: { cacheName?: string }): Promise<Response | undefined> {
    const cache = opts?.cacheName ? this.caches.get(opts.cacheName) : undefined;
    return cache?.match(req);
  }
}

function responseWith(size: number): Response {
  const body = ".".repeat(size);
  return new Response(body, {
    headers: { "content-type": "application/json", "content-length": String(size) },
  });
}

const ORIGIN = "https://easyquran.fyi";

function dataKey(index: number): string {
  return `/app/juz/${index}/__data.json`;
}

function urlFor(index: number): string {
  return `${ORIGIN}${dataKey(index)}`;
}

let fakeCaches: FakeCacheStorage;

function delay(ms = 2): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function putEntry(index: number, size: number): Promise<void> {
  const cache = await fakeCaches.open(DATA_CACHE);
  await cache.put(new Request(urlFor(index)), responseWith(size));
  await recordDataEntry(dataKey(index), responseWith(size));
}

async function touchIndex(index: number): Promise<void> {
  await delay();
  await touchDataMeta(dataKey(index));
}

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
  await purgeAllDataMeta();
});

afterEach(async () => {
  await purgeAllDataMeta();
  vi.unstubAllGlobals();
});

describe("eq-data-v1 metadata is one record per normalized key", () => {
  it("recordDataEntry stores {key,lastUsed,sizeBytes} keyed by the normalized data key", async () => {
    await recordDataEntry(dataKey(1), responseWith(128));
    const entry = await readDataMeta(dataKey(1));
    expect(entry).toBeDefined();
    expect(entry?.key).toBe(dataKey(1));
    expect(entry?.sizeBytes).toBe(128);
    expect(typeof entry?.lastUsed).toBe("number");
  });

  it("scanDataMeta returns one entry per normalized key", async () => {
    await putEntry(1, 10);
    await putEntry(2, 20);
    const all = await scanDataMeta();
    expect(all.size).toBe(2);
    expect(all.get(dataKey(1))?.sizeBytes).toBe(10);
    expect(all.get(dataKey(2))?.sizeBytes).toBe(20);
  });

  it("touchDataMeta updates only lastUsed for the touched key, preserving size", async () => {
    await putEntry(1, 64);
    const before = (await readDataMeta(dataKey(1)))!.lastUsed;
    await touchIndex(1);
    const after = await readDataMeta(dataKey(1));
    expect(after?.sizeBytes).toBe(64);
    expect(after!.lastUsed).toBeGreaterThan(before);
  });
});

describe("eq-data-v1 eviction holds BOTH count and byte caps", () => {
  it("enforces the count cap DATA_MAX by evicting least-recent keys", async () => {
    for (let i = 1; i <= DATA_MAX + 5; i++) await putEntry(i, 10);
    await touchIndex(1);
    await touchIndex(2);
    await touchIndex(3);

    await enforceDataBoundsInner();

    const cache = await fakeCaches.open(DATA_CACHE);
    const keys = await cache.keys();
    expect(keys.length).toBeLessThanOrEqual(DATA_MAX);
    const survivorKeys = new Set(keys.map((r) => normalizeDataKey(r.url)));
    expect(survivorKeys.has(dataKey(1))).toBe(true);
    expect(survivorKeys.has(dataKey(2))).toBe(true);
    expect(survivorKeys.has(dataKey(3))).toBe(true);
    const meta = await scanDataMeta();
    expect(meta.size).toBe(survivorKeys.size);
  });

  it("enforces the byte cap DATA_BUDGET_BYTES even when under the count cap", async () => {
    const big = Math.floor(DATA_BUDGET_BYTES / 3) + 1;
    for (let i = 1; i <= 4; i++) await putEntry(i, big);
    await touchIndex(1);
    await touchIndex(2);

    await enforceDataBoundsInner();

    const meta = await scanDataMeta();
    let total = 0;
    for (const e of meta.values()) total += e.sizeBytes;
    expect(total).toBeLessThanOrEqual(DATA_BUDGET_BYTES);
    expect(meta.has(dataKey(1))).toBe(true);
    expect(meta.has(dataKey(2))).toBe(true);
    expect(meta.size).toBeLessThanOrEqual(2);
  });
});

describe("concurrent touches preserve true LRU order", () => {
  it("records lastUsed in strict touch order across interleaved operations", async () => {
    await putEntry(1, 5);
    await putEntry(2, 5);
    await putEntry(3, 5);
    await putEntry(4, 5);
    await putEntry(5, 5);
    await touchIndex(1);
    await touchIndex(5);
    await touchIndex(3);

    const meta = await scanDataMeta();
    const ordered = [...meta.values()].sort((a, b) => a.lastUsed - b.lastUsed);
    expect(ordered[0]?.key).toBe(dataKey(2));
    expect(ordered[1]?.key).toBe(dataKey(4));
    expect(ordered.at(-1)?.key).toBe(dataKey(3));
  });
});

describe("startup reconciliation drops orphan metadata and counts missing metadata", () => {
  it("drops metadata records whose cache entry is gone (orphan meta)", async () => {
    await recordDataEntry(dataKey(1), responseWith(10));
    await recordDataEntry(dataKey(2), responseWith(10));
    expect((await scanDataMeta()).size).toBe(2);

    await enforceDataBoundsInner();

    expect((await scanDataMeta()).size).toBe(0);
  });

  it("keeps cache entries that are missing metadata and evicts them last by count", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    for (let i = 1; i <= DATA_MAX + 2; i++) {
      await cache.put(new Request(urlFor(i)), responseWith(1));
    }
    await enforceDataBoundsInner();
    const keys = await cache.keys();
    expect(keys.length).toBeLessThanOrEqual(DATA_MAX);
  });
});

describe("every deletion path clears matching metadata", () => {
  it("deleteDataMeta removes a single key's metadata", async () => {
    await recordDataEntry(dataKey(7), responseWith(10));
    await deleteDataMeta(dataKey(7));
    expect(await readDataMeta(dataKey(7))).toBeUndefined();
  });

  it("purgeAllDataMeta removes every data metadata record (cache reset path)", async () => {
    await putEntry(1, 10);
    await putEntry(2, 10);
    await purgeAllDataMeta();
    expect((await scanDataMeta()).size).toBe(0);
  });

  it("ordinary eviction removes metadata alongside the cache entry", async () => {
    for (let i = 1; i <= DATA_MAX + 1; i++) await putEntry(i, 8);
    await enforceDataBoundsInner();
    const cache = await fakeCaches.open(DATA_CACHE);
    const meta = await scanDataMeta();
    const cacheKeys = new Set((await cache.keys()).map((r) => normalizeDataKey(r.url)));
    expect(meta.size).toBe(cacheKeys.size);
    for (const key of meta.keys()) expect(cacheKeys.has(key)).toBe(true);
  });

  it("W7 pending eviction (cache delete + deleteDataMeta) leaves no orphan metadata", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    await cache.put(new Request(urlFor(3)), responseWith(20));
    await recordDataEntry(dataKey(3), responseWith(20));
    expect((await scanDataMeta()).has(dataKey(3))).toBe(true);

    await cache.delete(new Request(urlFor(3)));
    await deleteDataMeta(dataKey(3));

    await enforceDataBoundsInner();
    const meta = await scanDataMeta();
    expect(meta.has(dataKey(3))).toBe(false);
    expect((await cache.keys()).length).toBe(0);
  });

  it("a failed cache.put during handleData revalidation clears the key's metadata (W6 step 7)", async () => {
    await recordDataEntry(dataKey(5), responseWith(40));
    expect(await readDataMeta(dataKey(5))).toBeDefined();

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(responseWith(40)));

    const cache = await fakeCaches.open(DATA_CACHE);
    const putSpy = vi.spyOn(cache, "put").mockRejectedValue(new Error("quota exceeded"));

    await handleData(new Request(urlFor(5)));

    expect(putSpy).toHaveBeenCalledTimes(1);
    expect(await readDataMeta(dataKey(5))).toBeUndefined();
    expect((await cache.keys()).length).toBe(0);
  });
});

describe("offline-pack entries live in their separate cache and do not count", () => {
  it("enforceDataBounds never touches a pack cache", async () => {
    const pack = await fakeCaches.open("eq-pack-pack-xyz");
    await pack.put(new Request(urlFor(1)), responseWith(100));
    await enforceDataBoundsInner();
    const survivors = await pack.keys();
    expect(survivors.length).toBe(1);
  });

  it("pack entries are not represented in data metadata", async () => {
    await putEntry(1, 10);
    const pack = await fakeCaches.open("eq-pack-pack-abc");
    await pack.put(new Request(urlFor(99)), responseWith(9999));
    await enforceDataBoundsInner();
    const meta = await scanDataMeta();
    for (const k of meta.keys()) expect(k.includes("pack")).toBe(false);
  });
});

describe("enforceDataBounds serializes against concurrent metadata writes", () => {
  it("a concurrent burst of recordDataEntry + enforceDataBounds settles without throwing", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    await cache.put(new Request(urlFor(1)), responseWith(1));
    const ops: Promise<unknown>[] = [];
    for (let i = 1; i <= 20; i++) {
      ops.push(recordDataEntry(dataKey(i), responseWith(1)));
      ops.push(enforceDataBounds());
    }
    await Promise.all(ops);
    const meta = await scanDataMeta();
    expect(meta.size).toBeGreaterThanOrEqual(1);
  });
});

describe("W8a private/no-store responses stay out of eq-data-v1", () => {
  it("isCacheable rejects private so a private __data.json can never be recorded or stored", () => {
    expect(isCacheable(responseWith(16))).toBe(true);
    expect(
      isCacheable(new Response("x", { headers: { "cache-control": "private, no-store" } })),
    ).toBe(false);
    expect(isCacheable(new Response("x", { headers: { "cache-control": "Private" } }))).toBe(false);
    expect(
      isCacheable(new Response("x", { headers: { "cache-control": "private, max-age=60" } })),
    ).toBe(false);
  });

  it("keeps public, non-pending responses cacheable so anonymous data still fills the bucket", () => {
    expect(
      isCacheable(new Response("x", { headers: { "cache-control": "public, max-age=60" } })),
    ).toBe(true);
    expect(isCacheable(new Response("x", { headers: { "cache-control": "no-cache" } }))).toBe(true);
  });
});
