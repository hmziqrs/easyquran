import type { ActivePack } from "$lib/offline/meta";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { metaMock } = vi.hoisted(() => ({
  metaMock: {
    getActivePack: vi.fn<() => Promise<ActivePack | null>>(),
    setActivePack: vi.fn<(pack: ActivePack) => Promise<void>>(),
    clearActivePack: vi.fn<() => Promise<void>>(),
  },
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$lib/offline/meta", () => metaMock);

import { createOfflineStore } from "$lib/offline/offline-store.svelte";

const MIRROR_KEY = "easyquran.offline.pack";

interface FakeCache {
  keys: () => Promise<Request[]>;
  put: (req: Request, res: Response) => Promise<void>;
}

interface CachesApi {
  has(name: string): Promise<boolean>;
  open(name: string): Promise<FakeCache>;
  delete(name: string): Promise<boolean>;
}

function installFakeCaches(): CachesApi & {
  seed(name: string, count: number): void;
  throws(hasThrows: boolean): void;
  calls: { has: number; open: number; delete: number };
} {
  const stores = new Map<string, Request[]>();
  const calls = { has: 0, open: 0, delete: 0 };
  let hasThrows = false;
  const api = {
    calls,
    seed(name: string, count: number) {
      stores.set(
        name,
        Array.from({ length: count }, (_, i) => new Request(`https://t/${name}/${i}`)),
      );
    },
    throws(value: boolean) {
      hasThrows = value;
    },
    has: async (name: string): Promise<boolean> => {
      calls.has += 1;
      if (hasThrows) throw new Error("caches.has exploded");
      return stores.has(name);
    },
    open: async (name: string): Promise<FakeCache> => {
      calls.open += 1;
      let entry = stores.get(name);
      if (!entry) {
        entry = [];
        stores.set(name, entry);
      }
      return {
        keys: () => Promise.resolve([...entry!]),
        put: async () => {},
      };
    },
    delete: async (name: string): Promise<boolean> => {
      calls.delete += 1;
      return stores.delete(name);
    },
  };
  return api;
}

function flushMicrotasks(): Promise<void> {
  return Promise.resolve();
}

async function flushMany(): Promise<void> {
  for (let i = 0; i < 20; i++) await flushMicrotasks();
}

function installStorageEstimate(): void {
  Object.defineProperty(navigator, "storage", {
    value: { estimate: () => Promise.resolve({ usage: 1000, quota: 5000 }) },
    configurable: true,
  });
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  metaMock.getActivePack.mockReset();
  metaMock.setActivePack.mockReset();
  metaMock.clearActivePack.mockReset();
  metaMock.getActivePack.mockResolvedValue(null);
  metaMock.setActivePack.mockResolvedValue(undefined);
  metaMock.clearActivePack.mockResolvedValue(undefined);
  installStorageEstimate();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("OfflineStore #reconcile", () => {
  it("clears a stale mirror and resets to idle when no active pack record exists", async () => {
    const mirror = { packId: "abc", entries: 3, bytes: 99, savedAt: 1 };
    localStorage.setItem(MIRROR_KEY, JSON.stringify(mirror));
    metaMock.getActivePack.mockResolvedValue(null);
    const caches = installFakeCaches();
    vi.stubGlobal("caches", caches);

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    expect(store.status).toBe("idle");
    expect(store.activePack).toBeNull();
    expect(JSON.parse(localStorage.getItem(MIRROR_KEY) ?? "null")).toBeNull();
    expect(caches.calls.has).toBe(0);
    expect(metaMock.clearActivePack).not.toHaveBeenCalled();
  });

  it("treats a missing cache as incomplete and clears both meta and cache", async () => {
    const active: ActivePack = { packId: "abc", entries: 3, bytes: 99, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    vi.stubGlobal("caches", caches);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("not found", { status: 404 }));

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    expect(store.status).toBe("idle");
    expect(store.activePack).toBeNull();
    expect(metaMock.clearActivePack).toHaveBeenCalledTimes(1);
    expect(caches.calls.has).toBe(1);
    expect(caches.calls.delete).toBe(1);
  });

  it("marks the store active when the cache exists with the expected entry count", async () => {
    const active: ActivePack = { packId: "abc", entries: 3, bytes: 99, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    caches.seed("eq-pack-abc", 3);
    vi.stubGlobal("caches", caches);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response("not found", { status: 404 }));

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    expect(store.status).toBe("active");
    expect(store.activePack).toEqual(active);
    expect(metaMock.clearActivePack).not.toHaveBeenCalled();
  });

  it("treats a cache with a mismatched entry count as incomplete", async () => {
    const active: ActivePack = { packId: "abc", entries: 9, bytes: 99, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    caches.seed("eq-pack-abc", 2);
    vi.stubGlobal("caches", caches);

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    expect(store.status).toBe("idle");
    expect(store.activePack).toBeNull();
    expect(metaMock.clearActivePack).toHaveBeenCalledTimes(1);
    expect(caches.calls.delete).toBe(1);
  });

  it("retries reconcile after 30s when caches.has throws (unknown state)", async () => {
    vi.useFakeTimers();
    const active: ActivePack = { packId: "abc", entries: 3, bytes: 99, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    caches.throws(true);
    vi.stubGlobal("caches", caches);

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    expect(metaMock.getActivePack).toHaveBeenCalledTimes(1);
    expect(caches.calls.has).toBe(1);

    caches.throws(false);
    caches.seed("eq-pack-abc", 3);
    await vi.advanceTimersByTimeAsync(30_000);
    await flushMany();

    expect(metaMock.getActivePack).toHaveBeenCalledTimes(2);
    expect(caches.calls.has).toBe(2);
    expect(store.status).toBe("active");
  });
});

describe("OfflineStore.enable", () => {
  it("short-circuits to active without fetching the pack when the pack id is unchanged", async () => {
    const active: ActivePack = { packId: "abc", entries: 3, bytes: 50, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    caches.seed("eq-pack-abc", 3);
    vi.stubGlobal("caches", caches);
    const manifestBody = JSON.stringify({
      pack: "/offline/pack.abc.json",
      bytes: 50,
      entries: 3,
    });
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(manifestBody, { status: 200 }));

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();
    await store.enable();

    expect(store.status).toBe("active");
    const fetchedUrls = fetchSpy.mock.calls.map(([url]) => {
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- fetch's first argument is RequestInfo | URL; typeof-string is the only honest discrimination inside this spy mapper.
      return typeof url === "string" ? url : "";
    });
    expect(fetchedUrls.some((u) => u.includes("pack.abc.json"))).toBe(false);
  });

  it("reports an error when the streamed pack size does not match the manifest", async () => {
    const manifestBody = JSON.stringify({
      pack: "/offline/pack.new.json",
      bytes: 999,
      entries: 1,
    });
    const packBody = "tiny";
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- fetch stub's first argument is RequestInfo | URL; typeof-string discriminates the string case this stub serves.
      if (typeof url === "string" && url.includes("manifest.json")) {
        return Promise.resolve(new Response(manifestBody, { status: 200 }));
      }
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(packBody));
          controller.close();
        },
      });
      return Promise.resolve(new Response(stream, { status: 200 }));
    });
    const caches = installFakeCaches();
    vi.stubGlobal("caches", caches);

    const store = createOfflineStore();
    store.hydrate();
    await flushMany();

    await store.enable();

    expect(store.status).toBe("error");
    expect(fetchSpy).toHaveBeenCalled();
  });

  it("reuses bootCheck's manifest when the active pack is outdated", async () => {
    const active: ActivePack = { packId: "old", entries: 1, bytes: 5, savedAt: 1 };
    metaMock.getActivePack.mockResolvedValue(active);
    const caches = installFakeCaches();
    caches.seed("eq-pack-old", 1);
    vi.stubGlobal("caches", caches);
    const packBody = JSON.stringify({
      version: 1,
      entries: { "/app/1/__data.json": 0 },
      bodies: ["payload"],
    });
    const manifestBody = JSON.stringify({
      pack: "/offline/pack.new.json",
      bytes: new TextEncoder().encode(packBody).byteLength,
      entries: 1,
    });
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- fetch test double discriminates string RequestInfo supplied by OfflineStore
      if (typeof url === "string" && url.includes("manifest.json")) {
        return Promise.resolve(new Response(manifestBody, { status: 200 }));
      }
      return Promise.resolve(new Response(packBody, { status: 200 }));
    });

    const store = createOfflineStore();
    store.hydrate();
    await vi.waitFor(() => {
      expect(metaMock.setActivePack).toHaveBeenCalledTimes(1);
    });

    const fetchedUrls = fetchSpy.mock.calls.map(([url]) => {
      // eslint-disable-next-line anti-slop/no-runtime-typeof -- fetch spy records RequestInfo | URL; production calls under test use string URLs
      return typeof url === "string" ? url : "";
    });
    expect(fetchedUrls.filter((url) => url.includes("manifest.json"))).toHaveLength(1);
    expect(fetchedUrls.filter((url) => url.includes("pack.new.json"))).toHaveLength(1);
  });
});
