/// <reference types="@sveltejs/kit" />
/// <reference lib="webworker" />

import { base, build, files, version } from "$service-worker";
import {
  SKIP_WAITING,
  APP_READY,
  UPDATE_TAKEOVER,
  SW_BROADCAST_CHANNEL,
  type ClientToSwMessage,
  type SwToClientMessage,
} from "./lib/offline/messages";
import { openIdb, idbGet, idbPut, idbDelete } from "./lib/workers/idb";

const sw = self as unknown as ServiceWorkerGlobalScope;

const APP_CACHE = `eq-app-${version}`;
const PAGES_CACHE = "eq-pages-v1";
const DATA_CACHE = "eq-data-v1";

const META_DB = "easyquran-sw-meta";
const META_STORE = "meta";

const SHELL_ROUTE = `${base}/404.html`;
const NAV_TIMEOUT_MS = 3500;
const PAGES_MAX = 300;
const MAINTENANCE_CONCURRENCY = 6;

const PRECACHE = Array.from(
  new Set([...build, ...files, "/", "/app", `${base}/quran-meta/quran-data.json`]),
);

const IMMUTABLE = new Set([...build, ...files]);

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

let dbAvailable = true;

function db(): Promise<IDBDatabase> {
  if (!dbAvailable) return Promise.reject(new Error("idb unavailable"));
  return openIdb(META_DB, META_STORE);
}

async function metaGet<T>(key: string): Promise<T | undefined> {
  try {
    return await idbGet<T>(await db(), META_STORE, key);
  } catch {
    return undefined;
  }
}

async function metaSet(key: string, value: unknown): Promise<void> {
  try {
    await idbPut(await db(), META_STORE, value, key);
  } catch {
    dbAvailable = false;
  }
}

async function metaDel(key: string): Promise<void> {
  try {
    await idbDelete(await db(), META_STORE, key);
  } catch {
    dbAvailable = false;
  }
}

async function metaScan<T>(prefix: string): Promise<Record<string, T>> {
  const out: Record<string, T> = {};
  try {
    const database = await db();
    await new Promise<void>((resolve, reject) => {
      const tx = database.transaction(META_STORE, "readonly");
      const request = tx.objectStore(META_STORE).openCursor();
      request.onsuccess = () => {
        const cursor = request.result;
        if (!cursor) {
          resolve();
          return;
        }
        const key = cursor.key;
        if (typeof key === "string" && key.startsWith(prefix)) {
          out[key.slice(prefix.length)] = cursor.value as T;
        }
        cursor.continue();
      };
      request.onerror = () => reject(request.error);
    });
  } catch {}
  return out;
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
    const shellCtrl = new AbortController();
    const shellTimer = setTimeout(() => shellCtrl.abort(), 4000);
    const shell = await fetch(SHELL_ROUTE, {
      cache: "no-cache",
      signal: shellCtrl.signal,
    }).catch(() => null);
    clearTimeout(shellTimer);
    if (shell && shell.ok) await cache.put(SHELL_ROUTE, shell);
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
  const priorExisted = keys.some((k) => k.startsWith("eq-app-") && k !== APP_CACHE);
  const prev = await metaGet<string>("installedVersion");
  if (prev && prev !== version) {
    await setCursor(null);
  }
  await metaSet("installedVersion", version);
  if (priorExisted) announceTakeover();
}

function announceTakeover(): void {
  const message: SwToClientMessage = { type: UPDATE_TAKEOVER, version };
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

function isClientToSw(m: unknown): m is ClientToSwMessage {
  return !!m && typeof m === "object" && typeof (m as { type?: unknown }).type === "string";
}

sw.addEventListener("message", (event) => {
  if (!isClientToSw(event.data)) return;
  const data = event.data;
  switch (data.type) {
    case SKIP_WAITING:
      void sw.skipWaiting();
      return;
    case APP_READY:
      void onAppReady(event.source as Client | null);
      return;
  }
});

async function onAppReady(source: Client | null): Promise<void> {
  if (source) {
    await metaSet(`ack:${source.id}`, version);
  }
  await maybeFinalizeHandoff();
  void runMaintenance();
}

async function maybeFinalizeHandoff(): Promise<void> {
  const live = await sw.clients.matchAll({ includeUncontrolled: false });
  const liveIds = new Set(live.map((c) => c.id));
  const acks = await metaScan<string>("ack:");
  await Promise.all(
    Object.keys(acks)
      .filter((id) => !liveIds.has(id))
      .map((id) => metaDel(`ack:${id}`)),
  );
  if (live.length === 0) return;
  const allAcked = live.every((c) => acks[c.id] === version);
  if (!allAcked) return;
  await pruneOldAppCaches();
}

async function pruneOldAppCaches(): Promise<void> {
  const keys = await caches.keys();
  await Promise.all(
    keys.filter((k) => k.startsWith("eq-app-") && k !== APP_CACHE).map((k) => caches.delete(k)),
  );
}

sw.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  if (url.pathname.startsWith("/quran/")) return;
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

  if (url.origin === sw.location.origin && IMMUTABLE.has(url.pathname)) {
    event.respondWith(cacheFirstApp(req));
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

  const activePack = await metaGet<{ packId?: string } | null>("activePack");
  const packId = activePack?.packId;
  if (packId) {
    const packName = `eq-pack-${packId}`;
    if (await caches.has(packName)) {
      const packHit = await caches.match(new Request(key), {
        cacheName: packName,
        ignoreSearch: true,
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

type MaintenanceCursor =
  | { stage: "pages"; offset: number }
  | { stage: "data"; offset: number }
  | { stage: "trim" }
  | { stage: "done" }
  | null;

async function setCursor(cursor: MaintenanceCursor): Promise<void> {
  await metaSet("maintenance", { cursor });
}

function normalizeCursor(raw: unknown): MaintenanceCursor {
  if (raw && typeof raw === "object") {
    const c = raw as { stage?: unknown; offset?: unknown };
    if (c.stage === "trim") return { stage: "trim" };
    if (c.stage === "done") return { stage: "done" };
    if ((c.stage === "pages" || c.stage === "data") && typeof c.offset === "number") {
      return { stage: c.stage, offset: c.offset };
    }
  }
  return null;
}

async function runMaintenance(): Promise<void> {
  if (maintenanceInFlight) return;
  maintenanceInFlight = true;
  try {
    const meta = await metaGet<{ cursor: unknown }>("maintenance");
    let cursor: MaintenanceCursor = normalizeCursor(meta?.cursor);
    let successes = 0;
    let attempted = 0;
    for (;;) {
      if (cursor === null) {
        const r = await revalidateCache(PAGES_CACHE, 0, "pages");
        successes += r.successes;
        attempted += r.attempted;
        cursor = { stage: "data", offset: 0 };
        await setCursor(cursor);
        continue;
      }
      switch (cursor.stage) {
        case "pages": {
          const r = await revalidateCache(PAGES_CACHE, cursor.offset, "pages");
          successes += r.successes;
          attempted += r.attempted;
          cursor = { stage: "data", offset: 0 };
          await setCursor(cursor);
          continue;
        }
        case "data": {
          const r = await revalidateCache(DATA_CACHE, cursor.offset, "data");
          successes += r.successes;
          attempted += r.attempted;
          cursor = { stage: "trim" };
          await setCursor(cursor);
          continue;
        }
        case "trim": {
          await trimPages();
          if (attempted > 0 && successes === 0) {
            await setCursor(null);
            return;
          }
          cursor = { stage: "done" };
          await setCursor(cursor);
          continue;
        }
        case "done": {
          return;
        }
        default: {
          const _: never = cursor;
          void _;
          return;
        }
      }
    }
  } finally {
    maintenanceInFlight = false;
  }
}

async function revalidateCache(
  cacheName: string,
  start: number,
  stage: "pages" | "data",
): Promise<{ successes: number; attempted: number }> {
  const cache = await caches.open(cacheName);
  const all = await cache.keys();
  const slice = all.slice(start).map((req, i) => ({ req, idx: start + i }));
  const done = new Set<number>();
  let contiguous = start;
  let successes = 0;
  await pool(slice, MAINTENANCE_CONCURRENCY, async (item) => {
    let ok = false;
    try {
      const res = await fetch(item.req, { cache: "no-store" });
      if (res && res.ok && isCacheable(res)) {
        await cache.put(item.req, res.clone());
        ok = true;
      }
    } catch {
      // keep stale on failure
    }
    if (ok) successes++;
    done.add(item.idx);
    while (done.has(contiguous)) contiguous++;
    await setCursor({ stage, offset: contiguous });
  });
  return { successes, attempted: slice.length };
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
