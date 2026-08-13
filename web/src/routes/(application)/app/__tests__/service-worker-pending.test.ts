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

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "" } }));
vi.mock("$lib/data/quran", () => ({
  translationIdFromSegments: (lang: string, translator: string) => `${lang}.${translator}`,
}));
vi.mock("$lib/server/quran-data", () => ({
  QURAN_DATA: { rangeByIndex: () => ({ startGlobal: 1, endGlobal: 2 }) },
}));

const { disk } = vi.hoisted(() => ({
  disk: {
    diskCacheKey: () => "k",
    // SAFETY: cachedHtml is mutated between string and null across tests, so it is typed string | null; initialized null and reset in beforeEach.
    cachedHtml: null as string | null,
    reads: 0,
    // SAFETY: writes collects cache keys pushed across tests, so it is typed string[]; initialized empty and reset via .length = 0.
    writes: [] as string[],
  },
}));
vi.mock("$lib/server/quran-disk-cache", () => ({
  diskCacheKey: disk.diskCacheKey,
  getCachedHtml: async () => {
    disk.reads += 1;
    return disk.cachedHtml;
  },
  setCachedHtml: async (key: string) => {
    disk.writes.push(key);
  },
}));

import { applyHeaders, handle } from "../../../../hooks.server";
import {
  DATA_CACHE,
  deleteDataMeta,
  handleData,
  isCacheable,
  isPending,
  purgeAllDataMeta,
  readDataMeta,
  recordDataEntry,
} from "../../../../service-worker";

// happy-dom enforces forbidden header names — it drops `cookie` on Request
// construction and `set-cookie` on Response construction (getSetCookie() too) —
// so the cookie-aware impl path (correct for real browsers) cannot be exercised
// through the Headers API. Stub the header read at the seam instead.
function stubHeader(headers: Headers, name: string, value: string): void {
  const realGet = headers.get.bind(headers);
  Object.defineProperty(headers, "get", {
    configurable: true,
    value: (n: string) => (n.toLowerCase() === name ? value : realGet(n)),
  });
}

class FakeCache {
  readonly entries = new Map<string, Response>();

  async match(req: Request | string): Promise<Response | undefined> {
    const key = req instanceof Request ? new URL(req.url, "https://easyquran.fyi").pathname : req;
    return this.entries.get(key);
  }

  async put(req: Request | string, res: Response): Promise<void> {
    const key = req instanceof Request ? new URL(req.url, "https://easyquran.fyi").pathname : req;
    this.entries.set(key, res);
  }

  async delete(req: Request | string): Promise<boolean> {
    const key = req instanceof Request ? new URL(req.url, "https://easyquran.fyi").pathname : req;
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

const ORIGIN = "https://easyquran.fyi";

function dataKey(index: number): string {
  return `/app/juz/${index}/__data.json`;
}

function dataUrl(index: number): string {
  return `${ORIGIN}${dataKey(index)}`;
}

function successResponse(size = 64): Response {
  return new Response(".".repeat(size), {
    headers: { "content-type": "application/json", "content-length": String(size) },
  });
}

function pendingResponse(size = 64): Response {
  return new Response(".".repeat(size), {
    headers: {
      "content-type": "application/json",
      "content-length": String(size),
      "x-eq-translation-pending": "1",
      "cache-control": "no-store",
    },
  });
}

let fakeCaches: FakeCacheStorage;
let fetchMock: ReturnType<typeof vi.fn>;

function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 10));
}

beforeEach(async () => {
  memIdb.clear();
  disk.reads = 0;
  disk.writes.length = 0;
  disk.cachedHtml = null;
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
  fetchMock = vi.fn();
  vi.stubGlobal("fetch", fetchMock);
  await purgeAllDataMeta();
});

afterEach(async () => {
  await purgeAllDataMeta();
  vi.unstubAllGlobals();
});

describe("W7 pending/no-store detection", () => {
  it("isPending is true only for the x-eq-translation-pending marker", () => {
    expect(isPending(pendingResponse())).toBe(true);
    expect(isPending(successResponse())).toBe(false);
  });

  it("isCacheable rejects both no-store and pending responses", () => {
    expect(isCacheable(successResponse())).toBe(true);
    const noStore = new Response("x", { headers: { "cache-control": "no-store" } });
    expect(isCacheable(noStore)).toBe(false);
    expect(isCacheable(pendingResponse())).toBe(false);
  });
});

describe("W7 hooks force and preserve Cache-Control: no-store", () => {
  it("forces no-store on a response carrying the pending marker", () => {
    const res = new Response("x", { headers: { "x-eq-translation-pending": "1" } });
    applyHeaders(res, "/app/t/en/test/juz/5");
    expect(res.headers.get("cache-control")).toBe("no-store");
  });

  it("preserves an existing no-store instead of rewriting it to no-cache", () => {
    const res = new Response("x", { headers: { "cache-control": "no-store" } });
    applyHeaders(res, "/app/t/en/test/juz/5");
    expect(res.headers.get("cache-control")).toBe("no-store");
  });

  it("leaves a normal translated response cacheable as no-cache", () => {
    const res = new Response("x", {});
    applyHeaders(res, "/app/t/en/test/juz/5");
    expect(res.headers.get("cache-control")).toBe("no-cache");
  });
});

describe("W7 handleData never caches a pending response", () => {
  it("returns the pending response without writing the cache or metadata", async () => {
    fetchMock.mockResolvedValue(pendingResponse());

    const res = await handleData(new Request(dataUrl(5)));

    expect(res.ok).toBe(true);
    expect(isPending(res)).toBe(true);
    const cache = await fakeCaches.open(DATA_CACHE);
    expect((await cache.keys()).length).toBe(0);
    expect(await readDataMeta(dataKey(5))).toBeUndefined();
  });
});

describe("W7 handleData evicts an old pending cached entry and its metadata", () => {
  it("deletes a stale pending 200 and its W6 metadata, even while the network is still down", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    await cache.put(new Request(dataUrl(7)), pendingResponse());
    await recordDataEntry(dataKey(7), pendingResponse());
    expect(await readDataMeta(dataKey(7))).toBeDefined();

    fetchMock.mockRejectedValue(new Error("offline"));

    await handleData(new Request(dataUrl(7)));

    expect((await cache.keys()).length).toBe(0);
    expect(await readDataMeta(dataKey(7))).toBeUndefined();
  });

  it("evicts the stale pending entry, then caches the recovered success in the same lookup", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    await cache.put(new Request(dataUrl(7)), pendingResponse());
    await recordDataEntry(dataKey(7), pendingResponse());

    fetchMock.mockResolvedValue(successResponse());

    const res = await handleData(new Request(dataUrl(7)));

    expect(res.ok).toBe(true);
    expect(isPending(res)).toBe(false);
    const stored = await cache.match(new Request(dataUrl(7)));
    expect(stored).toBeDefined();
    expect(isPending(stored!)).toBe(false);
    expect(await readDataMeta(dataKey(7))).toBeDefined();
  });
});

describe("W7 a pending revalidation never overwrites or deletes a known-good entry", () => {
  it("keeps the successful cached response and its metadata when the network blips to pending", async () => {
    const cache = await fakeCaches.open(DATA_CACHE);
    await cache.put(new Request(dataUrl(9)), successResponse());
    await recordDataEntry(dataKey(9), successResponse());
    const before = await readDataMeta(dataKey(9));

    fetchMock.mockResolvedValue(pendingResponse());

    const res = await handleData(new Request(dataUrl(9)));
    expect(res.ok).toBe(true);
    expect(isPending(res)).toBe(false);

    await flush();

    const stored = await cache.match(new Request(dataUrl(9)));
    expect(stored).toBeDefined();
    expect(isPending(stored!)).toBe(false);
    const after = await readDataMeta(dataKey(9));
    expect(after).toBeDefined();
    expect(after?.sizeBytes).toBe(before?.sizeBytes);
    expect((await cache.keys()).length).toBe(1);
  });
});

describe("W7 API outage then recovery returns content on the first retry", () => {
  it("does not cache the first pending response; the next lookup caches and serves recovered content", async () => {
    fetchMock.mockResolvedValueOnce(pendingResponse());

    const first = await handleData(new Request(dataUrl(11)));
    expect(isPending(first)).toBe(true);
    const cache = await fakeCaches.open(DATA_CACHE);
    expect((await cache.keys()).length).toBe(0);

    fetchMock.mockResolvedValueOnce(successResponse());

    const second = await handleData(new Request(dataUrl(11)));
    expect(second.ok).toBe(true);
    expect(isPending(second)).toBe(false);
    const stored = await cache.match(new Request(dataUrl(11)));
    expect(stored).toBeDefined();
    expect(isPending(stored!)).toBe(false);
  });
});

describe("W7 pending cleanup is a metadata-clearing deletion path", () => {
  it("clears metadata for a key evicted as pending via deleteDataMeta", async () => {
    await recordDataEntry(dataKey(13), successResponse());
    expect(await readDataMeta(dataKey(13))).toBeDefined();

    await deleteDataMeta(dataKey(13));

    expect(await readDataMeta(dataKey(13))).toBeUndefined();
  });
});

describe("W8a isCacheable rejects private Cache-Control so the SW never stores auth-bound bytes", () => {
  it("rejects private and private, no-store; accepts public and no-cache", () => {
    expect(
      isCacheable(new Response("x", { headers: { "cache-control": "private, no-store" } })),
    ).toBe(false);
    expect(isCacheable(new Response("x", { headers: { "cache-control": "Private" } }))).toBe(false);
    expect(
      isCacheable(new Response("x", { headers: { "cache-control": "public, max-age=60" } })),
    ).toBe(true);
    expect(isCacheable(new Response("x", { headers: { "cache-control": "no-cache" } }))).toBe(true);
  });
});

describe("W8a applyHeaders isolates cookie-bearing and session-setting web responses", () => {
  it("forces private, no-store on a cookie-bearing document request", () => {
    const res = new Response("x");
    applyHeaders(res, "/app/t/en/test/juz/5", true);
    expect(res.headers.get("cache-control")).toBe("private, no-store");
  });

  it("forces private, no-store on a cookie-bearing __data.json response", () => {
    const res = new Response("x");
    applyHeaders(res, "/app/t/en/test/juz/5/__data.json", true);
    expect(res.headers.get("cache-control")).toBe("private, no-store");
  });

  it("forces private, no-store when the response sets a cookie, even with no request cookie", () => {
    const res = new Response("x");
    stubHeader(res.headers, "set-cookie", "ruxlog.sid=abc; HttpOnly");
    applyHeaders(res, "/app/t/en/test/juz/5", false);
    expect(res.headers.get("cache-control")).toBe("private, no-store");
  });

  it("keeps immutable static assets immutable even when the request carries a cookie", () => {
    const immutable = new Response("x");
    applyHeaders(immutable, "/_app/immutable/app.abcd1234.js", true);
    expect(immutable.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
    const tanzil = new Response("x");
    applyHeaders(tanzil, "/_quran/tanzil/en.test.json", true);
    expect(tanzil.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
  });

  it("leaves an anonymous, non-session-setting document cacheable as no-cache", () => {
    const res = new Response("x");
    applyHeaders(res, "/app/t/en/test/juz/5", false);
    expect(res.headers.get("cache-control")).toBe("no-cache");
  });
});

describe("W8a handle bypasses the SSR disk cache for cookie-bearing requests", () => {
  type HandleCall = Parameters<typeof handle>[0];

  function runHandle(
    opts: { pathname: string; cookie?: string; isDataRequest?: boolean },
    resolveResponse: Response,
  ): Promise<Response> {
    const url = new URL(`${ORIGIN}${opts.pathname}`);
    const request = new Request(url, { method: "GET" });
    if (opts.cookie) stubHeader(request.headers, "cookie", opts.cookie);
    const event = {
      request,
      url,
      route: { id: "/(application)/app/t/[lang]/[translator]/juz/[n]" },
      params: { lang: "en", translator: "sahih", n: "5" },
      isDataRequest: opts.isDataRequest ?? false,
    };
    const resolveStub: HandleCall["resolve"] = () => Promise.resolve(resolveResponse);
    const args: HandleCall = {
      // SAFETY: `event` is a partial RequestEvent carrying exactly the fields handle() reads (request/url/route/params/isDataRequest); widened through unknown because the real RequestEvent carries ~20 more members (locals/getClientAddress/platform/...) the handler never touches, so no single assertion compiles.
      // eslint-disable-next-line anti-slop/no-chained-type-assertions -- RequestEvent is a large SvelteKit interface the partial fake cannot structurally satisfy; collapse-to-one assertion does not compile, and widening handle()'s signature is out of this file's scope.
      event: event as unknown as HandleCall["event"],
      resolve: resolveStub,
    };
    return Promise.resolve(handle(args));
  }

  it("never reads from the disk cache when the request carries a cookie", async () => {
    disk.cachedHtml = "<html>cached anonymous</html>";
    const res = await runHandle(
      { pathname: "/en/app/t/en/sahih/juz/5", cookie: "ruxlog.sid=abc" },
      new Response("<html>fresh anonymous</html>", {
        headers: { "content-type": "text/html" },
      }),
    );
    expect(disk.reads).toBe(0);
    expect(await res.text()).toBe("<html>fresh anonymous</html>");
    expect(res.headers.get("cache-control")).toBe("private, no-store");
  });

  it("serves the disk cache hit for an anonymous request", async () => {
    disk.cachedHtml = "<html>cached anonymous</html>";
    const res = await runHandle(
      { pathname: "/en/app/t/en/sahih/juz/5" },
      new Response("<html>fresh</html>", { headers: { "content-type": "text/html" } }),
    );
    expect(disk.reads).toBe(1);
    expect(await res.text()).toBe("<html>cached anonymous</html>");
  });

  it("never writes the disk cache when the request carries a cookie", async () => {
    disk.cachedHtml = null;
    await runHandle(
      { pathname: "/en/app/t/en/sahih/juz/5", cookie: "ruxlog.sid=abc" },
      new Response("<html>fresh</html>", { headers: { "content-type": "text/html" } }),
    );
    expect(disk.writes).toHaveLength(0);
  });

  it("skips the disk write when an anonymous SSR response sets a cookie", async () => {
    disk.cachedHtml = null;
    const res = new Response("<html>fresh</html>", { headers: { "content-type": "text/html" } });
    stubHeader(res.headers, "set-cookie", "sid=rotated; HttpOnly");
    await runHandle({ pathname: "/en/app/t/en/sahih/juz/5" }, res);
    expect(disk.writes).toHaveLength(0);
  });

  it("writes the disk cache for an anonymous, non-session-setting document", async () => {
    disk.cachedHtml = null;
    await runHandle(
      { pathname: "/en/app/t/en/sahih/juz/5" },
      new Response("<html>fresh</html>", { headers: { "content-type": "text/html" } }),
    );
    expect(disk.writes).toEqual(["k__ui-en"]);
  });

  it("marks a cookie-bearing __data.json response private, no-store and bypasses the disk cache", async () => {
    disk.cachedHtml = "<html>cached</html>";
    const res = await runHandle(
      {
        pathname: "/en/app/t/en/sahih/juz/5/__data.json",
        cookie: "ruxlog.sid=abc",
        isDataRequest: true,
      },
      new Response("{}", { headers: { "content-type": "application/json" } }),
    );
    expect(disk.reads).toBe(0);
    expect(disk.writes).toHaveLength(0);
    expect(res.headers.get("cache-control")).toBe("private, no-store");
  });
});
