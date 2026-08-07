import type { Handle, RequestEvent } from "@sveltejs/kit";
import { translationIdFromSegments } from "$lib/data/quran";
import { QURAN } from "$lib/config/site";
import { QURAN_DATA } from "$lib/server/quran-data";
import { diskCacheKey, getCachedHtml, setCachedHtml } from "$lib/server/quran-disk-cache";

const IMMUTABLE = "public, max-age=31536000, immutable";

const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;

function translationRouteCacheKey(event: RequestEvent): string | null {
  const id = event.route.id;
  if (id === null || !id.includes("/t/[lang]/[translator]")) return null;
  const { lang, translator } = event.params;
  if (!lang || translator === undefined) return null;
  const sourceId = translationIdFromSegments(lang, translator);
  if (event.params.surah) {
    const surah = QURAN_DATA.surahBySlug(event.params.surah);
    if (!surah) return null;
    const localPage = event.params.localPage ? Number(event.params.localPage) : 1;
    if (!Number.isSafeInteger(localPage) || localPage < 1) return null;
    return diskCacheKey(sourceId, "surah", surah.num, localPage);
  }
  const index = Number(event.params.n);
  if (!Number.isSafeInteger(index) || index < 1) return null;
  return diskCacheKey(sourceId, id.includes("/juz/") ? "juz" : "page", index);
}

function buildCsp(): string {
  const api = QURAN.apiBase
    ? QURAN.apiBase.endsWith("/")
      ? QURAN.apiBase
      : QURAN.apiBase + "/"
    : "";
  const connectSrc = [
    "'self'",
    "https://*.firebaseio.com",
    "wss://*.firebaseio.com",
    "https://firestore.googleapis.com",
    "https://firebase.googleapis.com",
    "https://firebaselogging-pa.googleapis.com",
    "https://www.google-analytics.com",
    "https://www.google.com",
  ];
  if (api) connectSrc.push(api);
  if (import.meta.env.DEV) {
    connectSrc.push("http://localhost:*", "ws://localhost:*", "wss://localhost:*");
  }
  return [
    "default-src 'self'",
    "script-src 'self' 'unsafe-inline' https://www.gstatic.com https://www.googletagmanager.com",
    `connect-src ${connectSrc.join(" ")}`,
    "worker-src 'self' blob:",
    "img-src 'self' data: https:",
    "style-src 'self' 'unsafe-inline'",
    "object-src 'none'",
    "base-uri 'self'",
    "frame-ancestors 'none'",
  ].join("; ");
}

function applyHeaders(response: Response, pathname: string): void {
  response.headers.set("Content-Security-Policy", buildCsp());
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  if (response.status >= 500) {
    response.headers.set("Cache-Control", "no-store");
  } else if (
    pathname.startsWith("/_app/immutable/") ||
    pathname.startsWith("/_quran/tanzil/") ||
    packPattern.test(pathname)
  ) {
    response.headers.set("Cache-Control", IMMUTABLE);
  } else {
    response.headers.set("Cache-Control", "no-cache");
  }
  if (pathname.endsWith(".md") || pathname.endsWith(".txt")) {
    response.headers.set("X-Robots-Tag", "noindex, follow");
  }
}

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;
  const key = translationRouteCacheKey(event);
  const cacheable = event.request.method === "GET" && !event.isDataRequest && key !== null;

  if (cacheable) {
    const hit = await getCachedHtml(key);
    if (hit !== null) {
      const response = new Response(hit, {
        headers: {
          "content-type": "text/html; charset=utf-8",
          "server-timing": 'quran_ssr_cache;desc="hit"',
          "x-easyquran-quran-cache": "hit",
        },
      });
      applyHeaders(response, pathname);
      return response;
    }
    const response = await resolve(event);
    const contentType = response.headers.get("content-type") ?? "";
    if (
      response.status === 200 &&
      contentType.includes("text/html") &&
      !response.headers.get("x-eq-translation-pending")
    ) {
      const clone = response.clone();
      await clone
        .text()
        .then((html) => setCachedHtml(key, html))
        .catch(() => {});
    }
    response.headers.set("server-timing", 'quran_ssr_cache;desc="miss"');
    response.headers.set("x-easyquran-quran-cache", "miss");
    applyHeaders(response, pathname);
    return response;
  }

  const response = await resolve(event);
  applyHeaders(response, pathname);
  return response;
};
