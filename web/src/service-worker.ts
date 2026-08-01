/// <reference types="@sveltejs/kit" />
/// <reference lib="webworker" />

import { base, build, files, version } from "$service-worker";

const sw = self as unknown as ServiceWorkerGlobalScope;

const PRECACHE = `eq-precache-${version}`;
const RUNTIME = "eq-runtime-v1";
const KEEP = new Set([PRECACHE, RUNTIME]);

const OFFLINE_FALLBACK = `${base}/404.html`;
const PRECACHE_URLS = [...new Set([...build, ...files, OFFLINE_FALLBACK])];

const MATCH_OPTIONS: CacheQueryOptions = { ignoreSearch: true, ignoreVary: true };

async function cacheFirst(req: Request, cacheName: string): Promise<Response> {
  const cache = await caches.open(cacheName);
  const hit = await cache.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (res && res.ok) void cache.put(req, res.clone()).catch(() => {});
  return res;
}

async function staleWhileRevalidate(req: Request, cacheName: string): Promise<Response> {
  const cache = await caches.open(cacheName);
  const hit = await cache.match(req);
  const network = fetch(req)
    .then((res) => {
      if (res && res.ok) void cache.put(req, res.clone()).catch(() => {});
      return res;
    })
    .catch(() => null);
  const fresh = await network;
  return hit || fresh || Response.error();
}

async function networkFirstNav(req: Request): Promise<Response> {
  const cache = await caches.open(PRECACHE);
  try {
    const res = await fetch(req);
    if (res && res.ok && res.type !== "opaque") await cache.put(req, res.clone());
    return res;
  } catch {
    return (
      (await caches.match(req, MATCH_OPTIONS)) ||
      (await caches.match(OFFLINE_FALLBACK, MATCH_OPTIONS)) ||
      Response.error()
    );
  }
}

sw.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(PRECACHE);
      await Promise.allSettled(
        PRECACHE_URLS.map(async (url) => {
          const res = await fetch(url);
          if (res.ok) await cache.put(url, res);
        }),
      );
    })(),
  );
});

sw.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((k) => !KEEP.has(k)).map((k) => caches.delete(k)));
      await sw.clients.claim();
    })(),
  );
});

sw.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  if (url.pathname.startsWith("/quran/v1/")) return;
  if (url.pathname.startsWith("/_quran/")) return;
  if (url.hostname.endsWith("r2.easyquran.fyi")) return;
  if (url.pathname === "/firebase-config.js") return;
  if (url.pathname.includes("/translations/")) return;

  if (url.pathname.includes("/_app/immutable/")) {
    event.respondWith(cacheFirst(req, PRECACHE));
    return;
  }
  if (req.mode === "navigate") {
    event.respondWith(networkFirstNav(req));
    return;
  }
  if (url.origin === sw.location.origin) {
    event.respondWith(staleWhileRevalidate(req, RUNTIME));
  }
});

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
