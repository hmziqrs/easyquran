import type { Handle, RequestEvent } from "@sveltejs/kit";
import { building } from "$app/environment";
import { QURAN } from "$lib/config/site";
import { isUiLocale, uiDirection, type UiDirection, type UiLocale } from "$lib/i18n/locales";
import { paraglideMiddleware } from "$lib/paraglide/server";
import { diskCacheKey, getCachedHtml, setCachedHtml } from "$lib/server/quran-disk-cache";
import {
  localizedReaderLocale,
  parseReaderPath,
  parseReaderRoute,
  type ParsedReaderRoute,
} from "$lib/server/reader-route";

const IMMUTABLE = "public, max-age=31536000, immutable";

const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;

function translationRouteCacheKey(
  route: ParsedReaderRoute | null,
  uiLocale: UiLocale | null,
): string | null {
  if (!uiLocale || route?.type !== "translation") return null;
  const base = diskCacheKey(
    route.sourceId,
    route.cacheKind,
    route.index,
    route.cacheKind === "surah" ? (route.localPage ?? 1) : undefined,
  );
  return `${base}__ui-${uiLocale}`;
}

function withTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

function buildCsp(): string {
  const api = QURAN.apiBase ? withTrailingSlash(QURAN.apiBase) : "";
  const connectSrc = [
    "'self'",
    "https://*.firebaseio.com",
    "wss://*.firebaseio.com",
    "https://firestore.googleapis.com",
    "https://firebase.googleapis.com",
    "https://firebaseinstallations.googleapis.com",
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

export function hasNoStore(response: Response): boolean {
  const cc = response.headers.get("cache-control");
  return !!cc && /no-store/i.test(cc);
}

function responseSetsCookie(response: Response): boolean {
  if (response.headers.get("set-cookie")) return true;
  const getter = (response.headers as Headers & { getSetCookie?: () => string[] }).getSetCookie;
  return typeof getter === "function" && getter.call(response.headers).length > 0;
}

export function applyHeaders(response: Response, pathname: string, requestHasCookie = false): void {
  response.headers.set("Content-Security-Policy", buildCsp());
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  const translationPending = response.headers.get("x-eq-translation-pending");
  const privateMode = requestHasCookie || responseSetsCookie(response);
  const isImmutableAsset =
    pathname.startsWith("/_app/immutable/") ||
    pathname.startsWith("/_quran/tanzil/") ||
    packPattern.test(pathname);
  if (privateMode && !isImmutableAsset) {
    response.headers.set("Cache-Control", "private, no-store");
  } else if (response.status >= 500 || hasNoStore(response) || translationPending) {
    response.headers.set("Cache-Control", "no-store");
  } else if (isImmutableAsset) {
    response.headers.set("Cache-Control", IMMUTABLE);
  } else {
    response.headers.set("Cache-Control", "no-cache");
  }
  if (pathname.endsWith(".md") || pathname.endsWith(".txt")) {
    response.headers.set("X-Robots-Tag", "noindex, follow");
  }
}

interface DocumentAttributes {
  readonly lang: string;
  readonly dir: UiDirection;
}

function documentAttributes(
  route: ParsedReaderRoute | null,
  uiLocale: UiLocale | null,
): DocumentAttributes {
  if (route?.type === "translation") {
    return { lang: route.contentLanguage, dir: route.contentDirection };
  }
  if (route?.type === "arabic") return { lang: "ar", dir: "rtl" };
  if (uiLocale) return { lang: uiLocale, dir: uiDirection(uiLocale) };
  return { lang: "en", dir: "ltr" };
}

function transformRootHtml(html: string, attributes: DocumentAttributes): string {
  return html
    .replace('lang="%lang%"', `lang="${attributes.lang}"`)
    .replace('dir="%dir%"', `dir="${attributes.dir}"`);
}

function legacyReaderRedirect(event: RequestEvent): Response | null {
  const { pathname } = event.url;
  if (!pathname.startsWith("/app") || !parseReaderPath(pathname)) return null;
  const tail = building ? "" : `${event.url.search}${event.url.hash}`;
  return new Response(null, {
    status: 307,
    headers: {
      location: `/en${pathname}${tail}`,
      "cache-control": "no-store",
    },
  });
}

function noncanonicalLocalizedReaderRedirect(event: RequestEvent): Response | null {
  const { pathname } = event.url;
  if (
    event.params.localPage !== "1" ||
    !event.route.id ||
    (!event.route.id.endsWith("/app/[surah]/page/[localPage]") &&
      !event.route.id.endsWith("/app/[surah]/t/[lang]/[translator]/page/[localPage]")) ||
    !pathname.endsWith("/page/1")
  ) {
    return null;
  }
  const localizedCanonical = pathname.slice(0, -"/page/1".length);
  const canonical = localizedCanonical.replace(/^\/(?:en|ar)(?=\/app(?:\/|$))/u, "");
  if (!parseReaderPath(canonical)) return null;
  const tail = building ? "" : `${event.url.search}${event.url.hash}`;
  return new Response(null, {
    status: 308,
    headers: { location: `${localizedCanonical}${tail}` },
  });
}

function notFound(): Response {
  return new Response("Not found", {
    status: 404,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}

async function resolveRequest(
  event: RequestEvent,
  resolve: Parameters<Handle>[0]["resolve"],
  uiLocale: UiLocale | null,
  readerRoute: ParsedReaderRoute | null,
  requestHasCookie: boolean,
): Promise<Response> {
  const key = translationRouteCacheKey(readerRoute, uiLocale);
  const cacheable =
    event.request.method === "GET" && !event.isDataRequest && key !== null && !requestHasCookie;
  const attributes = documentAttributes(readerRoute, uiLocale);
  const resolveOpts = {
    transformPageChunk: ({ html }: { html: string }) => transformRootHtml(html, attributes),
  };

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
      return response;
    }
    const response = await resolve(event, resolveOpts);
    const contentType = response.headers.get("content-type") ?? "";
    const setsCookie = responseSetsCookie(response);
    if (
      !setsCookie &&
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
    return response;
  }

  return resolve(event, resolveOpts);
}

function isLocalizedMarketingPath(pathname: string): boolean {
  return pathname === "/" || pathname === "/ar" || pathname === "/ar/";
}

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;
  const requestHasCookie = !!event.request.headers.get("cookie");
  let response = legacyReaderRedirect(event);

  if (!response) {
    const readerLocale = localizedReaderLocale(pathname);
    const useI18n = readerLocale !== null || isLocalizedMarketingPath(pathname);
    const noncanonicalRedirect = readerLocale ? noncanonicalLocalizedReaderRedirect(event) : null;
    if (noncanonicalRedirect) {
      response = noncanonicalRedirect;
    } else if (readerLocale && !parseReaderRoute(event.route.id, event.params)) {
      response = notFound();
    } else if (useI18n) {
      response = await paraglideMiddleware(event.request, async ({ request, locale }) => {
        if (!isUiLocale(locale)) return notFound();
        event.request = request;
        const readerRoute = readerLocale ? parseReaderRoute(event.route.id, event.params) : null;
        return resolveRequest(event, resolve, locale, readerRoute, requestHasCookie);
      });
    } else {
      response = await resolveRequest(
        event,
        resolve,
        null,
        parseReaderRoute(event.route.id, event.params),
        requestHasCookie,
      );
    }
  }

  applyHeaders(response, pathname, requestHasCookie);
  return response;
};
