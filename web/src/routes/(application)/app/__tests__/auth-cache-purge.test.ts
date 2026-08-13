import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$service-worker", () => ({
  base: "",
  build: [] as string[],
  files: [] as string[],
  version: "test-v1",
}));

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

import { PURGE_ACK, PURGE_USER_CACHES, purgeUserCaches } from "../../../../lib/offline/messages";
import {
  DATA_CACHE,
  PAGES_CACHE,
  purgeAllDataMeta,
  purgeUserCachesHandler,
  recordDataEntry,
  scanDataMeta,
} from "../../../../service-worker";

class FakeCache {
  readonly entries = new Map<string, Response>();

  async match(req: Request | string): Promise<Response | undefined> {
    const key = typeof req === "string" ? req : new URL(req.url, ORIGIN).pathname;
    return this.entries.get(key);
  }

  async put(req: Request | string, res: Response): Promise<void> {
    const key = typeof req === "string" ? req : new URL(req.url, ORIGIN).pathname;
    this.entries.set(key, res);
  }

  async delete(req: Request | string): Promise<boolean> {
    const key = typeof req === "string" ? req : new URL(req.url, ORIGIN).pathname;
    return this.entries.delete(key);
  }

  async keys(): Promise<Request[]> {
    return [...this.entries.keys()].map((k) => new Request(`${ORIGIN}${k}`));
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

interface FakePort {
  postMessage(msg: unknown): void;
  close(): void;
  onmessage: ((ev: { data: unknown }) => void) | null;
  start(): void;
}

function fakePort(): { port: MessagePort; messages: unknown[] } {
  const messages: unknown[] = [];
  const port: FakePort = {
    postMessage: (msg: unknown) => {
      messages.push(msg);
    },
    close: () => {},
    onmessage: null,
    start: () => {},
  };
  return { port: port as unknown as MessagePort, messages };
}

function flush(ms = 10): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
  await purgeAllDataMeta();
});

afterEach(async () => {
  await purgeAllDataMeta();
  vi.unstubAllGlobals();
});

describe("W8a purgeUserCachesHandler deletes pages, data, and W6 metadata, then acks", () => {
  it("removes eq-pages-v1, eq-data-v1, and all data metadata, posting PURGE_ACK on the port", async () => {
    const pages = await fakeCaches.open(PAGES_CACHE);
    await pages.put(new Request(`${ORIGIN}/app/t/en/test/juz/5`), successResponse(100));
    const data = await fakeCaches.open(DATA_CACHE);
    await data.put(new Request(dataUrl(5)), successResponse(40));
    await recordDataEntry(dataKey(5), successResponse(40));
    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(true);
    expect(fakeCaches.caches.has(DATA_CACHE)).toBe(true);
    expect((await scanDataMeta()).size).toBe(1);

    const { port, messages } = fakePort();
    await purgeUserCachesHandler(port);

    expect(messages).toHaveLength(1);
    expect((messages[0] as { type: string }).type).toBe(PURGE_ACK);
    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(false);
    expect(fakeCaches.caches.has(DATA_CACHE)).toBe(false);
    expect((await scanDataMeta()).size).toBe(0);
  });

  it("leaves the offline pack, OPFS-backed app shell, and unrelated caches untouched", async () => {
    const pack = await fakeCaches.open("eq-pack-xyz");
    await pack.put(new Request(`${ORIGIN}/app/t/en/test/juz/5`), successResponse(200));
    const app = await fakeCaches.open("eq-app-test-v1");
    await app.put(new Request(`${ORIGIN}/offline-shell`), successResponse(50));
    const data = await fakeCaches.open(DATA_CACHE);
    await data.put(new Request(dataUrl(7)), successResponse(30));
    await recordDataEntry(dataKey(7), successResponse(30));

    await purgeUserCachesHandler();

    expect((await pack.keys()).length).toBe(1);
    expect((await app.keys()).length).toBe(1);
    expect(fakeCaches.caches.has(DATA_CACHE)).toBe(false);
    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(false);
    expect((await scanDataMeta()).size).toBe(0);
    expect(fakeCaches.caches.has("eq-pack-xyz")).toBe(true);
    expect(fakeCaches.caches.has("eq-app-test-v1")).toBe(true);
  });

  it("purges even when no MessagePort is supplied (no ack, no throw)", async () => {
    const pages = await fakeCaches.open(PAGES_CACHE);
    await pages.put(new Request(`${ORIGIN}/app/t/en/test/juz/3`), successResponse(10));

    await expect(purgeUserCachesHandler()).resolves.toBeUndefined();
    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(false);
  });

  it("always acks after attempting the purge even if a bucket is already absent", async () => {
    const { port, messages } = fakePort();
    await purgeUserCachesHandler(port);
    expect(messages).toHaveLength(1);
    expect((messages[0] as { type: string }).type).toBe(PURGE_ACK);
  });
});

describe("W8a purgeUserCaches client helper", () => {
  function installFakeMessageChannel(): void {
    vi.stubGlobal("MessageChannel", function MessageChannel(): {
      port1: FakePort;
      port2: FakePort;
    } {
      const port1: FakePort = {
        postMessage: () => {},
        close: () => {},
        onmessage: null,
        start: () => {},
      };
      const port2: FakePort = {
        postMessage: (msg: unknown) => {
          port1.onmessage?.({ data: msg });
        },
        close: () => {},
        onmessage: null,
        start: () => {},
      };
      return { port1, port2 };
    });
  }

  function stubController(controller: unknown): void {
    vi.stubGlobal("navigator", { serviceWorker: { controller } });
  }

  it("posts PURGE_USER_CACHES on the controller with a transferred port and resolves on ack", async () => {
    installFakeMessageChannel();
    let sent: { msg: unknown; port: FakePort | null } | null = null;
    const controller = {
      postMessage(msg: unknown, transfer: unknown[]): void {
        sent = { msg, port: (transfer[0] as FakePort) ?? null };
      },
    };
    stubController(controller);

    const promise = purgeUserCaches(5000);
    await flush(5);

    expect(sent).not.toBeNull();
    expect((sent!.msg as { type: string }).type).toBe(PURGE_USER_CACHES);
    expect(sent!.port).not.toBeNull();

    sent!.port!.postMessage({ type: PURGE_ACK });

    await expect(promise).resolves.toBeUndefined();
  });

  it("resolves on its own when the worker never acks (timeout fallback)", async () => {
    installFakeMessageChannel();
    let posted = false;
    const controller = {
      postMessage(): void {
        posted = true;
      },
    };
    stubController(controller);

    await expect(purgeUserCaches(20)).resolves.toBeUndefined();
    expect(posted).toBe(true);
  });

  it("no-ops without posting when no controller is attached", async () => {
    installFakeMessageChannel();
    let posted = false;
    stubController({
      postMessage(): void {
        posted = true;
      },
    });
    vi.stubGlobal("navigator", { serviceWorker: { controller: null } });

    await expect(purgeUserCaches(20)).resolves.toBeUndefined();
    expect(posted).toBe(false);
  });

  it("no-ops when navigator.serviceWorker is undefined", async () => {
    vi.stubGlobal("navigator", {});
    await expect(purgeUserCaches(20)).resolves.toBeUndefined();
  });
});

describe("W8a PURGE_USER_CACHES message wiring", () => {
  it("dispatching PURGE_USER_CACHES on the worker scope triggers the purge handler", async () => {
    const pages = await fakeCaches.open(PAGES_CACHE);
    await pages.put(new Request(`${ORIGIN}/app/t/en/test/juz/2`), successResponse(12));
    const data = await fakeCaches.open(DATA_CACHE);
    await data.put(new Request(dataKey(2)), successResponse(8));
    await recordDataEntry(dataKey(2), successResponse(8));

    self.dispatchEvent(new MessageEvent("message", { data: { type: PURGE_USER_CACHES } }));

    await flush(20);

    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(false);
    expect(fakeCaches.caches.has(DATA_CACHE)).toBe(false);
    expect((await scanDataMeta()).size).toBe(0);
  });

  it("ignores messages whose data fails the ClientToSwMessage guard", async () => {
    const pages = await fakeCaches.open(PAGES_CACHE);
    await pages.put(new Request(`${ORIGIN}/app/t/en/test/juz/9`), successResponse(5));

    self.dispatchEvent(new MessageEvent("message", { data: { unexpected: true } }));
    await flush(10);

    expect(fakeCaches.caches.has(PAGES_CACHE)).toBe(true);
  });
});
