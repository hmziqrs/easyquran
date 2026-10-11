import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({
  browser: false,
  dev: false,
  building: false,
  version: "test-v1",
}));

vi.mock("$app/manifest", () => ({
  // SAFETY: $app/manifest mock; kit 3 emits entry paths WITHOUT a leading
  // slash, so "favicon.png" doubles as the normalization fixture while
  // "robots.txt" doubles as the named precache-exclusion fixture. ".DS_Store"
  // (dotfile) and "quran-meta/translations.json" (catalogue prefix) exercise
  // the other two filter branches; "quran-meta/quran-data.json" is excluded by
  // that same filter and returns only via the deliberate PRECACHE re-add.
  assets: [
    { path: "robots.txt" },
    { path: ".DS_Store" },
    { path: "quran-meta/translations.json" },
    { path: "quran-meta/quran-data.json" },
  ],
  immutable: [{ path: "favicon.png" }],
  prerendered: [],
  routes: [],
}));

vi.mock("$app/paths", () => ({
  // base is "" in every deployment, so resolve()/asset() mock as a plain "/" prefix.
  resolve: (id: string) => `/${id}`,
  asset: (id: string) => `/${id}`,
  match: async () => null,
}));

// happy-dom has no persistent IDB; route every meta read/write through memory.
const { memIdb } = vi.hoisted(() => ({ memIdb: new Map<string, Map<string, unknown>>() }));

vi.mock("../../../lib/workers/idb", () => ({
  IDB_VERSION: 1,
  openIdb: async (db: string, store: string) => ({ db, store }),
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks lib/workers/idb idbGet; key mirrors the opaque IndexedDB key contract (callers live in service-worker.ts, outside this cluster)
  idbGet: async (h: { db: string; store: string }, _store: string, key: unknown) =>
    // SAFETY: the SW only ever passes string data keys; the inner memIdb map is string-keyed
    memIdb.get(`${h.db} ${h.store}`)?.get(key as string),
  idbPut: async (
    h: { db: string; store: string },
    _store: string,
    // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks lib/workers/idb idbPut; value is the opaque IDB payload stored verbatim by the SW
    value: unknown,
    // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks lib/workers/idb idbPut; key mirrors the opaque IndexedDB string key contract
    key?: unknown,
  ) => {
    const k = `${h.db} ${h.store}`;
    if (!memIdb.has(k)) memIdb.set(k, new Map());
    if (key !== undefined) {
      // SAFETY: the SW only ever passes string data keys; the inner memIdb map is string-keyed
      memIdb.get(k)!.set(key as string, value);
    }
  },
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks lib/workers/idb idbDelete; key mirrors the opaque IndexedDB key contract
  idbDelete: async (h: { db: string; store: string }, _store: string, key: unknown) => {
    // SAFETY: the SW only ever passes string data keys; the inner memIdb map is string-keyed
    memIdb.get(`${h.db} ${h.store}`)?.delete(key as string);
  },
  idbScan: async (h: { db: string; store: string }, _store: string, prefix: string) => {
    // eslint-disable-next-line anti-slop/no-unsafe-dictionary-type -- idbScan returns deserialized IDB values of unknown shape; the SW re-parses per consumer
    const out: Record<string, unknown> = {};
    const storeMap = memIdb.get(`${h.db} ${h.store}`);
    if (storeMap) {
      for (const [k, v] of storeMap) {
        if (k.startsWith(prefix)) out[k.slice(prefix.length)] = v;
      }
    }
    return out;
  },
}));

vi.mock("#lib/config/site.js", () => ({ QURAN: { apiBase: "" } }));
vi.mock("#lib/data/quran.js", () => ({ translationIdFromSegments: () => "en.test" }));
vi.mock("#lib/server/quran-data.js", () => ({ QURAN_DATA: {} }));

// Importing the module registers the top-level fetch listener on self.
await import("../../../service-worker");

class FakeCache {
  readonly entries = new Map<string, Response>();
  puts = 0;

  async match(req: Request | string): Promise<Response | undefined> {
    const key = req instanceof Request ? new URL(req.url).pathname : req;
    return this.entries.get(key);
  }
  async put(req: Request | string, res: Response): Promise<void> {
    this.puts++;
    const key = req instanceof Request ? new URL(req.url).pathname : req;
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
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks FetchEvent.respondWith; captures whatever response promise the SW handler resolves with
  respondWith(r: unknown): void {
    this.respondWithCalled = true;
    this.response = r;
  }
}

// A minimal InstallEvent: the SW handler hands precache()'s promise to
// .waitUntil, which the test then awaits.
class FakeInstallEvent extends Event {
  promise: Promise<void> | undefined;

  constructor() {
    super("install");
  }
  // eslint-disable-next-line anti-slop/no-unknown-parameters -- mocks InstallEvent.waitUntil; captures whatever promise the SW handler hands over
  waitUntil(p: unknown): void {
    // SAFETY: the SW install handler hands precache()'s Promise<void> to waitUntil;
    // `p` is unknown only because the mock's InstallEvent capture slot is untyped.
    this.promise = p as Promise<void>;
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
    const ev = dispatchFetch(makeRequest("/api/quran/sources/en.sahih/range?from=1&to=7"));
    expect(ev.respondWithCalled).toBe(false);
  });

  it("leaves every Cache Storage bucket empty after an /api/ GET", async () => {
    dispatchFetch(makeRequest("/api/quran/sources/en.sahih/range?from=1&to=7"));
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
    "/api/quran/search?q=mercy",
    "/api/quran/sources/en.sahih/surah/1",
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
    const ev = dispatchFetch(makeRequest("/api/quran/search", { method: "POST" }));
    expect(ev.respondWithCalled).toBe(false);
  });
});

describe("kit 3 manifest entries normalize to leading-slash, filtered precache", () => {
  it("precache() installs manifest entries under slash-prefixed keys and drops excluded files", async () => {
    const ev = new FakeInstallEvent();
    self.dispatchEvent(ev);
    await ev.promise;
    const app = fakeCaches.caches.get("eq-app-test-v1");
    expect(app).toBeDefined();
    const keys = [...app!.entries.keys()];
    // Kit 3 emits "favicon.png" (no slash); the precache key must be "/favicon.png".
    expect(keys).toContain("/favicon.png");
    expect(keys).not.toContain("favicon.png");
    // The hand-added entries normalize through the same pathname branch.
    expect(keys).toContain("/");
    expect(keys).toContain("/surah");
    // robots.txt (named exclusion), .DS_Store (dotfile), and quran-meta/**
    // (catalogue prefix) are all filtered out of the precache — a dotfile or
    // catalogue entry that leaked through would 4xx/fail at install time.
    expect(keys).not.toContain("/robots.txt");
    expect(keys).not.toContain("/.DS_Store");
    expect(keys).not.toContain("/quran-meta/translations.json");
    // quran-data.json IS precached, but only through the deliberate re-add in
    // PRECACHE (service-worker.ts) — the filter above excluded it.
    expect(keys).toContain("/quran-meta/quran-data.json");
  });

  it("serves a precached manifest asset cache-first with no background revalidate", async () => {
    const app = await fakeCaches.open("eq-app-test-v1");
    await app.put("/favicon.png", new Response("cached-favicon"));
    const ev = dispatchFetch(makeRequest("/favicon.png"));
    expect(ev.respondWithCalled).toBe(true);
    // SAFETY: the fetch handler passed cacheFirstApp()'s Response to respondWith();
    // ev.response is unknown only because the mock's capture slot is untyped.
    const res = (await ev.response) as Response;
    expect(await res.text()).toBe("cached-favicon");
    await flush();
    // Cache-first must not fire the SWR background fetch (the kit 2-era bug:
    // IMMUTABLE held "favicon.png" so "/favicon.png" never matched and every
    // static asset revalidated per request).
    expect(vi.mocked(fetch)).not.toHaveBeenCalled();
  });
});
