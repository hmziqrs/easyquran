import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$service-worker", () => ({
  base: "",
  build: [] as string[],
  files: [] as string[],
  version: "test-v1",
}));

// happy-dom has no persistent IDB; route every meta read/write through memory.
const { memIdb } = vi.hoisted(() => ({ memIdb: new Map<string, Map<string, unknown>>() }));

vi.mock("../../../../lib/workers/idb", () => ({
  IDB_VERSION: 1,
  openIdb: async (db: string, store: string) => ({ db, store }),
  idbGet: async (h: { db: string; store: string }, _store: string, key: unknown) =>
    memIdb.get(`${h.db} ${h.store}`)?.get(key as string),
  idbPut: async (
    h: { db: string; store: string },
    _store: string,
    value: unknown,
    key?: unknown,
  ) => {
    const k = `${h.db} ${h.store}`;
    if (!memIdb.has(k)) memIdb.set(k, new Map());
    if (key !== undefined) memIdb.get(k)!.set(key as string, value);
  },
  idbDelete: async (h: { db: string; store: string }, _store: string, key: unknown) => {
    memIdb.get(`${h.db} ${h.store}`)?.delete(key as string);
  },
  idbScan: async (h: { db: string; store: string }, _store: string, prefix: string) => {
    const out: Record<string, unknown> = {};
    const storeMap = memIdb.get(`${h.db} ${h.store}`);
    if (storeMap) {
      for (const [k, v] of storeMap) {
        if (typeof k === "string" && k.startsWith(prefix)) out[k.slice(prefix.length)] = v;
      }
    }
    return out;
  },
}));

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "" } }));
vi.mock("$lib/data/quran", () => ({ translationIdFromSegments: () => "en.test" }));
vi.mock("$lib/server/quran-data", () => ({ QURAN_DATA: {} }));

// Importing the module registers the top-level fetch listener on self.
await import("../../../../service-worker");

class FakeCache {
  readonly entries = new Map<string, Response>();
  puts = 0;

  async match(req: Request | string): Promise<Response | undefined> {
    const key = typeof req === "string" ? req : new URL(req.url).pathname;
    return this.entries.get(key);
  }
  async put(req: Request | string, res: Response): Promise<void> {
    this.puts++;
    const key = typeof req === "string" ? req : new URL(req.url).pathname;
    this.entries.set(key, res);
  }
  async delete(_req: Request | string): Promise<boolean> {
    return false;
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
}

// A minimal FetchEvent: the SW handler reads .request.method / .url / .mode and
// calls .respondWith with the response promise. We capture whether it was called.
class FakeFetchEvent extends Event {
  readonly request: Request;
  respondWithCalled = false;
  response: unknown = undefined;

  constructor(request: Request) {
    super("fetch");
    this.request = request;
  }
  respondWith(r: unknown): void {
    this.respondWithCalled = true;
    this.response = r;
  }
}

const ORIGIN = self.location.origin;

function makeRequest(pathname: string, init?: RequestInit): Request {
  return new Request(`${ORIGIN}${pathname}`, init);
}

function dispatchFetch(req: Request): FakeFetchEvent {
  const ev = new FakeFetchEvent(req);
  self.dispatchEvent(ev);
  return ev;
}

async function flush(): Promise<void> {
  for (let i = 0; i < 12; i++) await Promise.resolve();
  await new Promise<void>((r) => setTimeout(r, 10));
}

let fakeCaches: FakeCacheStorage;

function totalPuts(): number {
  let n = 0;
  for (const c of fakeCaches.caches.values()) n += c.puts;
  return n;
}

beforeEach(() => {
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
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("api body")));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("W5 same-origin /api/ requests never enter Cache Storage", () => {
  it("does not call respondWith for a same-origin /api/ GET (bypass)", async () => {
    const ev = dispatchFetch(makeRequest("/api/quran/sources"));
    expect(ev.respondWithCalled).toBe(false);
  });

  it("leaves every Cache Storage bucket empty after an /api/ GET", async () => {
    dispatchFetch(makeRequest("/api/quran/sources"));
    await flush();
    expect(totalPuts()).toBe(0);
    for (const name of fakeCaches.caches.keys()) {
      // No eq-app-*/eq-pages-v1/eq-data-v1 bucket should hold /api/ bytes.
      expect(name.startsWith("eq-app-") || name === "eq-pages-v1" || name === "eq-data-v1").toBe(
        true,
      );
      const cache = fakeCaches.caches.get(name)!;
      expect((await cache.keys()).length).toBe(0);
    }
  });

  it.each([
    "/api/quran/scripts",
    "/api/quran/sources",
    "/api/auth/session",
    "/api/quran/v1/surah/1",
  ])("bypasses %s regardless of path depth", (pathname) => {
    const ev = dispatchFetch(makeRequest(pathname));
    expect(ev.respondWithCalled).toBe(false);
  });

  it("still handles a non-/api/ same-origin GET through respondWith (control)", async () => {
    // A plain same-origin path falls through to swrApp(), proving the listener
    // is wired and the bypass is specific to /api/.
    const ev = dispatchFetch(makeRequest("/some-app-route"));
    expect(ev.respondWithCalled).toBe(true);
  });

  it("ignores non-GET /api/ requests at the top of the handler (method guard)", () => {
    const ev = dispatchFetch(makeRequest("/api/quran/sources", { method: "POST" }));
    expect(ev.respondWithCalled).toBe(false);
  });
});
