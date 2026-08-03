/// <reference types="@sveltejs/kit" />
/// <reference lib="webworker" />

import { base, build, files, version } from "$service-worker";

const sw = self as unknown as ServiceWorkerGlobalScope;

const SKIP_WAITING = "SKIP_WAITING";
const APP_READY = "APP_READY";
const UPDATE_TAKEOVER = "UPDATE_TAKEOVER";
const SW_BROADCAST_CHANNEL = "easyquran-sw";

const APP_CACHE = `eq-app-${version}`;
const PAGES_CACHE = "eq-pages-v1";
const DATA_CACHE = "eq-data-v1";
const LEGACY_RUNTIME = "eq-runtime-v1";

const META_DB = "easyquran-sw-meta";
const META_STORE = "meta";

const SHELL_ROUTE = `${base}/404.html`;
const NAV_TIMEOUT_MS = 3500;
const PAGES_MAX = 300;
const MAINTENANCE_CONCURRENCY = 6;

const PRECACHE = Array.from(
  new Set([
    ...build,
    ...files,
    "/",
    "/app",
    SHELL_ROUTE,
    `${base}/quran-meta/quran-data.json`,
  ]),
);

function normalizeDataKey(url: string | URL): string {
  const u = typeof url === "string" ? new URL(url, sw.location.origin) : url;
  const params = new URLSearchParams();
  for (const [key, value] of u.searchParams) {
    if (key.startsWith("x-sveltekit-")) continue;
    params.append(key, value);
  }
  const search = params.size > 0 ? `?${params.toString()}` : "";
  return `${u.pathname}${search}`;
}

function isCacheable(res: Response): boolean {
  const cc = res.headers.get("cache-control");
  if (cc && /no-store/i.test(cc)) return false;
  return true;
}

function isHtmlResponse(res: Response): boolean {
  const ct = res.headers.get("content-type") || "";
  return ct.includes("text/html");
}

let dbPromise: Promise<IDBDatabase> | null = null;
let dbAvailable = true;

function openMetaDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(META_DB, 1);
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(META_STORE)) {
        database.createObjectStore(META_STORE);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function db(): Promise<IDBDatabase> {
  if (!dbAvailable) return Promise.reject(new Error("idb unavailable"));
  dbPromise ??= openMetaDB();
  return dbPromise;
}

async function metaGet<T>(key: string): Promise<T | undefined> {
  try {
    const database = await db();
    return await new Promise<T | undefined>((resolve, reject) => {
      const tx = database.transaction(META_STORE, "readonly");
      const request = tx.objectStore(META_STORE).get(key);
      request.onsuccess = () => resolve(request.result as T | undefined);
      request.onerror = () => reject(request.error);
    });
  } catch {
    return undefined;
  }
}

async function metaSet(key: string, value: unknown): Promise<void> {
  try {
    const database = await db();
    await new Promise<void>((resolve, reject) => {
      const tx = database.transaction(META_STORE, "readwrite");
      tx.objectStore(META_STORE).put(value, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    dbAvailable = false;
  }
}

async function metaDel(key: string): Promise<void> {
  try {
    const database = await db();
    await new Promise<void>((resolve, reject) => {
      const tx = database.transaction(META_STORE, "readwrite");
      tx.objectStore(META_STORE).delete(key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } catch {
    dbAvailable = false;
  }
}

let broadcastChannel: BroadcastChannel | null = null;
function getBroadcast(): BroadcastChannel | null {
  if (broadcastChannel) return broadcastChannel;
  try {
    broadcastChannel = new BroadcastChannel(SW_BROADCAST_CHANNEL);
  } catch {
    broadcastChannel = null;
  }
  return broadcastChannel;
}

async function pool<T>(
  items: T[],
  limit: number,
  worker: (item: T, index: number) => Promise<void>,
): Promise<void> {
  let next = 0;
  async function run(): Promise<void> {
    while (true) {
      const i = next;
      next++;
      if (i >= items.length) return;
      await worker(items[i], i);
    }
  }
  const runners: Promise<void>[] = [];
  for (let k = 0; k < Math.min(limit, items.length); k++) runners.push(run());
  await Promise.all(runners);
}

sw.addEventListener("install", (event) => {
  event.waitUntil(precache());
});

async function precache(): Promise<void> {
  try {
    const cache = await caches.open(APP_CACHE);
    await Promise.all(
      PRECACHE.map(async (url) => {
        const res = await fetch(url, { cache: "no-cache" });
        if (!res || !res.ok) {
          throw new Error(`precache ${url} failed: ${res ? res.status : "no response"}`);
        }
        await cache.put(url, res);
      }),
    );
  } catch (err) {
    await caches.delete(APP_CACHE).catch(() => {});
    throw err;
  }
}

sw.addEventListener("activate", (event) => {
  event.waitUntil(activate());
});

async function activate(): Promise<void> {
  await sw.clients.claim();
  const keys = await caches.keys();
  const priorExisted = keys.some((k) => /^eq-(app|precache)-/.test(k) && k !== APP_CACHE);
  const prev = await metaGet<string>("installedVersion");
  if (prev && prev !== version) {
    await metaSet("maintenance", { cursor: null });
  }
  await metaSet("installedVersion", version);
  if (priorExisted) announceTakeover();
}

function announceTakeover(): void {
  const message = { type: UPDATE_TAKEOVER, version };
  void sw.clients
    .matchAll({ includeUncontrolled: true })
    .then((clients) => {
      for (const client of clients) client.postMessage(message);
    })
    .catch(() => {});
  const channel = getBroadcast();
  if (channel) {
    try {
      channel.postMessage(message);
    } catch {}
  }
}

sw.addEventListener("message", (event) => {
  const data = event.data;
  if (!data || typeof data.type !== "string") return;
  if (data.type === SKIP_WAITING) {
    void sw.skipWaiting();
    return;
  }
  if (data.type === APP_READY) {
    void onAppReady(event.source as Client | null);
  }
});

async function onAppReady(source: Client | null): Promise<void> {
  if (source) {
    const acks = (await metaGet<Record<string, string>>("acks")) || {};
    acks[source.id] = version;
    await metaSet("acks", acks);
  }
  await maybeFinalizeHandoff();
  void runMaintenance();
}

async function maybeFinalizeHandoff(): Promise<void> {
  const live = await sw.clients.matchAll({ includeUncontrolled: false });
  const liveIds = new Set(live.map((c) => c.id));
  const acks = (await metaGet<Record<string, string>>("acks")) || {};
  const pruned: Record<string, string> = {};
  for (const [id, v] of Object.entries(acks)) {
    if (liveIds.has(id)) pruned[id] = v;
  }
  await metaSet("acks", pruned);
  if (live.length === 0) return;
  const allAcked = live.every((c) => pruned[c.id] === version);
  if (!allAcked) return;
  await migrateLegacyCaches();
}

async function migrateLegacyCaches(): Promise<void> {
  const before = await caches.keys();
  const legacy = before.filter((k) => /^eq-precache-/.test(k) || k === LEGACY_RUNTIME);
  if (legacy.length > 0) {
    const pages = await caches.open(PAGES_CACHE);
    const data = await caches.open(DATA_CACHE);
    for (const name of legacy) {
      const cache = await caches.open(name);
      const requests = await cache.keys();
      for (const req of requests) {
        const res = await cache.match(req);
        if (!res || !res.ok) continue;
        const url = new URL(req.url);
        if (url.origin !== sw.location.origin) continue;
        if (url.pathname.endsWith("/__data.json")) {
          const key = normalizeDataKey(req.url);
          if (!(await data.match(key))) {
            await data.put(new Request(key), res.clone()).catch(() => {});
          }
        } else if (isHtmlResponse(res) && !url.pathname.startsWith("/_app/")) {
          const key = normalizeDataKey(req.url);
          if (!(await pages.match(key))) {
            await pages.put(new Request(key), res.clone()).catch(() => {});
          }
        }
      }
    }
  }
  const after = await caches.keys();
  await Promise.all(
    after
      .filter(
        (k) =>
          /^eq-precache-/.test(k) ||
          k === LEGACY_RUNTIME ||
          (/^eq-app-/.test(k) && k !== APP_CACHE),
      )
      .map((k) => caches.delete(k)),
  );
}

sw.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  if (url.pathname.startsWith("/quran/v1/")) return;
  if (url.pathname.startsWith("/_quran/")) return;
  if (url.hostname.endsWith("r2.easyquran.fyi")) return;
  if (url.pathname === "/firebase-config.js") return;
  if (url.pathname.includes("/translations/")) return;

  if (
    url.pathname === "/_app/version.json" ||
    url.pathname === "/service-worker.js" ||
    url.pathname === "/offline/manifest.json" ||
    (url.pathname.startsWith("/offline/pack.") && url.pathname.endsWith(".json"))
  ) {
    return;
  }

  if (url.pathname.startsWith("/_app/immutable/")) {
    event.respondWith(cacheFirstApp(req));
    return;
  }

  if (req.mode === "navigate") {
    event.respondWith(handleNavigation(req));
    return;
  }

  if (url.origin === sw.location.origin && url.pathname.endsWith("/__data.json")) {
    event.respondWith(handleData(req));
    return;
  }

  if (url.origin === sw.location.origin) {
    event.respondWith(swrApp(req));
  }
});

async function cacheFirstApp(req: Request): Promise<Response> {
  const cache = await caches.open(APP_CACHE);
  const hit = await cache.match(req);
  if (hit) return hit;
  try {
    const res = await fetch(req);
    if (res && res.ok && isCacheable(res)) {
      void cache.put(req, res.clone()).catch(() => {});
    }
    return res;
  } catch {
    return Response.error();
  }
}

async function handleNavigation(req: Request): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), NAV_TIMEOUT_MS);
  let network: Response | null = null;
  try {
    network = await fetch(req, { signal: controller.signal });
  } catch {
    network = null;
  } finally {
    clearTimeout(timeout);
  }

  if (network && network.ok && network.type !== "opaque" && isCacheable(network)) {
    const pages = await caches.open(PAGES_CACHE);
    await pages.put(req, network.clone()).catch(() => {});
    await touchRecency(req.url);
    return network;
  }
  if (network) return network;

  const app = await caches.open(APP_CACHE);
  const shellHit = await app.match(req);
  if (shellHit) {
    await touchRecency(req.url);
    return shellHit;
  }

  const pages = await caches.open(PAGES_CACHE);
  const pageHit = await pages.match(req);
  if (pageHit) {
    await touchRecency(req.url);
    return pageHit;
  }

  const fallback = await app.match(SHELL_ROUTE);
  return fallback || Response.error();
}

async function handleData(req: Request): Promise<Response> {
  const key = normalizeDataKey(req.url);
  const data = await caches.open(DATA_CACHE);
  const hit = await data.match(key);

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), NAV_TIMEOUT_MS);
  const revalidate = fetch(req, { signal: controller.signal })
    .then(async (res) => {
      if (res && res.ok && isCacheable(res)) {
        await data.put(new Request(key), res.clone()).catch(() => {});
      }
      return res;
    })
    .catch(() => null as Response | null)
    .finally(() => clearTimeout(timeout));

  if (hit) {
    void revalidate;
    return hit;
  }

  const fresh = await revalidate;
  if (fresh && fresh.ok) return fresh;

  const activePack = await metaGet<{ hash?: string } | null>("activePack");
  const hash = activePack?.hash;
  if (hash) {
    const packName = `eq-pack-${hash}`;
    if (await caches.has(packName)) {
      const packHit = await caches.match(new Request(key), {
        cacheName: packName,
        ignoreSearch: false,
      });
      if (packHit) return packHit;
    }
  }
  return Response.error();
}

async function swrApp(req: Request): Promise<Response> {
  const cache = await caches.open(APP_CACHE);
  const hit = await cache.match(req);
  const network = fetch(req)
    .then(async (res) => {
      if (res && res.ok && isCacheable(res)) {
        await cache.put(req, res.clone()).catch(() => {});
      }
      return res;
    })
    .catch(() => null as Response | null);
  if (hit) {
    void network;
    return hit;
  }
  const fresh = await network;
  return fresh || Response.error();
}

async function touchRecency(rawUrl: string): Promise<void> {
  const key = normalizeDataKey(rawUrl);
  const recency = (await metaGet<Record<string, number>>("recency")) || {};
  recency[key] = Date.now();
  await metaSet("recency", recency);
}

let maintenanceInFlight = false;

async function runMaintenance(): Promise<void> {
  if (maintenanceInFlight) return;
  maintenanceInFlight = true;
  try {
    const meta = (await metaGet<{ cursor: string | null }>("maintenance")) || {
      cursor: null,
    };
    let cursor = meta.cursor;
    for (;;) {
      if (cursor === "done") {
        await metaSet("maintenance", { cursor: "done" });
        return;
      }
      if (cursor == null || cursor.startsWith("pages:")) {
        const start = cursor ? Number(cursor.slice("pages:".length)) || 0 : 0;
        await revalidateCache(PAGES_CACHE, start, "pages");
        cursor = "data:0";
        await metaSet("maintenance", { cursor: "data:0" });
        continue;
      }
      if (cursor.startsWith("data:")) {
        const start = Number(cursor.slice("data:".length)) || 0;
        await revalidateCache(DATA_CACHE, start, "data");
        cursor = "trim:0";
        await metaSet("maintenance", { cursor: "trim:0" });
        continue;
      }
      if (cursor.startsWith("trim:")) {
        await trimPages();
        cursor = "done";
        await metaSet("maintenance", { cursor: "done" });
        continue;
      }
      cursor = "done";
      await metaSet("maintenance", { cursor: "done" });
      return;
    }
  } finally {
    maintenanceInFlight = false;
  }
}

async function revalidateCache(cacheName: string, start: number, tag: string): Promise<void> {
  const cache = await caches.open(cacheName);
  const all = await cache.keys();
  const slice = all.slice(start).map((req, i) => ({ req, idx: start + i }));
  const done = new Set<number>();
  let contiguous = start;
  await pool(slice, MAINTENANCE_CONCURRENCY, async (item) => {
    try {
      const res = await fetch(item.req, { cache: "no-store" });
      if (res && res.ok && isCacheable(res)) {
        await cache.put(item.req, res.clone());
      }
    } catch {
      // keep stale on failure
    }
    done.add(item.idx);
    while (done.has(contiguous)) contiguous++;
    await metaSet("maintenance", { cursor: `${tag}:${contiguous}` });
  });
}

async function trimPages(): Promise<void> {
  const cache = await caches.open(PAGES_CACHE);
  const reqs = await cache.keys();
  if (reqs.length <= PAGES_MAX) return;
  const recency = (await metaGet<Record<string, number>>("recency")) || {};
  const scored = reqs.map((req) => ({
    req,
    last: recency[normalizeDataKey(req.url)] ?? 0,
  }));
  scored.sort((a, b) => a.last - b.last);
  const evict = scored.slice(0, reqs.length - PAGES_MAX);
  const nextRecency = { ...recency };
  for (const { req } of evict) {
    await cache.delete(req).catch(() => {});
    const key = normalizeDataKey(req.url);
    delete nextRecency[key];
  }
  await metaSet("recency", nextRecency);
}

interface PushPayload {
  notification?: { title?: string; body?: string };
  data?: Record<string, string>;
}

sw.addEventListener("push", (event) => {
  let payload: PushPayload = {};
  try {
    payload = event.data ? (event.data.json() as PushPayload) : {};
  } catch {
    payload = {};
  }
  const n = payload.notification || {};
  const data = payload.data || {};
  const title = n.title || data.title || "EasyQuran";
  const body = n.body || data.body || "";
  const url = data.url || "/";
  event.waitUntil(
    sw.registration.showNotification(title, {
      body,
      icon: "/icons/icon-192.png",
      badge: "/icons/icon-192.png",
      tag: data.tag || undefined,
      data: Object.assign({ url }, data),
    }),
  );
});

sw.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target: string = event.notification?.data?.url || "/";
  event.waitUntil(
    (async () => {
      const all = await sw.clients.matchAll({ type: "window", includeUncontrolled: true });
      for (const client of all) {
        await client.focus();
        try {
          await client.navigate(target);
        } catch {
          void 0;
        }
        return;
      }
      await sw.clients.openWindow(target);
    })(),
  );
});
