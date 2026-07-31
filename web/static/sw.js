/* ════════════════════════════════════════════════════════════════════════
   sw.js — the ONE root Service Worker for EasyQuran (docs §9).

   Self-contained (NO importScripts): app-shell + asset caching, navigation
   preload, the documented bypass rules, and Firebase Cloud Messaging push
   handled natively. Folding FCM in here (instead of a separate
   firebase-messaging-sw.js) gives a single worker at scope "/".

   Why no importScripts of the gstatic FCM SDK? A top-level cross-origin
   importScripts that fails (CDN down, blocked, offline at install) aborts the
   whole worker — which would take offline reading down with it. The push
   payload is self-contained, so background display needs no SDK: the client
   uses getToken (firebase/messaging) to subscribe; THIS worker just receives
   the push and shows it. (firebase-config.js is no longer needed in the worker
   — the config lives client-side for token registration.)

   Bypass (never Cache-Storage these):
     • /quran/v1/**            — the live API (when up)
     • r2.easyquran.fyi        — the two Arabic SQLite files (OPFS owns them)
     • /firebase-config.js     — network-only so FCM config can't go stale
     • /translations/**        — translation routes (live-only per doc §11)

   Caching:
     • /_app/immutable/**      — cache-first, forever (hashed, immutable)
     • navigations (HTML)      — network-first; offline → cached page, then 404
     • other same-origin GETs  — stale-while-revalidate (icons, fonts, manifest)
   ════════════════════════════════════════════════════════════════════════ */

const IMMUTABLE = "eq-immutable-v1";
const RUNTIME = "eq-runtime-v1";
const NAV = "eq-nav-v1";
const KEEP = new Set([IMMUTABLE, RUNTIME, NAV]);

// ── caching strategies ─────────────────────────────────────────────────
async function cacheFirst(req, cacheName) {
  const cache = await caches.open(cacheName);
  const hit = await cache.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (res && res.ok) void cache.put(req, res.clone()).catch(() => {});
  return res;
}

async function staleWhileRevalidate(req, cacheName) {
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

async function networkFirstNav(req) {
  const cache = await caches.open(NAV);
  try {
    const res = await fetch(req);
    if (res && res.ok && res.type !== "opaque") await cache.put(req, res.clone());
    return res;
  } catch {
    return (await cache.match(req)) || (await cache.match("/404.html")) || Response.error();
  }
}

// ── lifecycle ──────────────────────────────────────────────────────────
self.addEventListener("install", (event) => {
  event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(keys.filter((k) => !KEEP.has(k)).map((k) => caches.delete(k)));
      await self.clients.claim();
    })(),
  );
});

// ── fetch routing ──────────────────────────────────────────────────────
self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);

  // Bypass rules.
  if (url.pathname.startsWith("/quran/v1/")) return;
  if (url.hostname.endsWith("r2.easyquran.fyi")) return;
  if (url.pathname === "/firebase-config.js") return;
  if (url.pathname.includes("/translations/")) return;

  // Immutable hashed assets: cache-first, forever.
  if (url.pathname.includes("/_app/immutable/")) {
    event.respondWith(cacheFirst(req, IMMUTABLE));
    return;
  }
  // HTML navigations: network-first, fall back to cached page then 404.html.
  if (req.mode === "navigate") {
    event.respondWith(networkFirstNav(req));
    return;
  }
  // Other same-origin static: stale-while-revalidate.
  if (url.origin === self.location.origin) {
    event.respondWith(staleWhileRevalidate(req, RUNTIME));
  }
});

// ── Firebase Cloud Messaging: native push display ─────────────────────
// FCM delivers a JSON payload with either a `notification` (display) or `data`
// (data-only) member. We show one notification either way; the click target is
// payload.data.url (defaulting to "/").
self.addEventListener("push", (event) => {
  let payload = {};
  try {
    payload = event.data ? event.data.json() : {};
  } catch {
    /* non-JSON payload — show a default */
  }
  const n = payload.notification || {};
  const data = payload.data || {};
  const title = n.title || data.title || "EasyQuran";
  const body = n.body || data.body || "";
  const url = data.url || "/";
  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      icon: "/icons/icon-192.png",
      badge: "/icons/icon-192.png",
      tag: data.tag || undefined,
      data: Object.assign({ url }, data),
    }),
  );
});

// Open/focus the right tab on click.
self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target =
    (event.notification && event.notification.data && event.notification.data.url) || "/";
  event.waitUntil(
    (async () => {
      const all = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
      for (const client of all) {
        if ("focus" in client) {
          await client.focus();
          if ("navigate" in client) {
            try {
              await client.navigate(target);
            } catch {
              /* focus is enough */
            }
          }
          return;
        }
      }
      if (self.clients.openWindow) await self.clients.openWindow(target);
    })(),
  );
});
