import { randomBytes } from "node:crypto";

import { building } from "$app/env";
import type { RequestEvent } from "@sveltejs/kit";
import type { Handle } from "@sveltejs/kit/hooks";

import { QURAN } from "#lib/config/site.js";
import { SURAH_COUNT } from "#lib/data/quran-data.js";
import { isUiLocale, uiDirection, type UiDirection, type UiLocale } from "#lib/i18n/locales.js";
import {
  MARKETING_PATHS,
  MARKETING_PUBLICATIONS,
  type MarketingPageId,
} from "#lib/i18n/marketing.js";
import { paraglideMiddleware } from "#lib/paraglide/server.js";
import {
  agentNotFoundMarkdown,
  appendVaryAccept,
  mdSiblingRequest,
  notAcceptableBody,
  preferredType,
} from "#lib/server/markdown-negotiation.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";
import { diskCacheKey, getCachedHtml, setCachedHtml } from "#lib/server/quran-disk-cache.js";
import {
  localizedReaderLocale,
  parseReaderRoute,
  surahLocalRedirectTarget,
  type ParsedReaderRoute,
} from "#lib/server/reader-route.js";

const IMMUTABLE = "public, max-age=31536000, immutable";

const NEGOTIABLE_TYPES = ["text/html", "text/markdown"];

const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;

const NONCE_PLACEHOLDER = "%csp-nonce%";

function freshNonce(): string {
  return randomBytes(16).toString("base64");
}

function translationRouteCacheKey(
  route: ParsedReaderRoute | null,
  uiLocale: UiLocale | null,
): string | null {
  if (!uiLocale || route?.type !== "translation") return null;
  // Bounded UI-locale partition on top of the source-kind key: en/ar chrome
  // renders different shells over the same translation content.
  return `${diskCacheKey(route.sourceId, route.cacheKind, route.index)}__ui-${uiLocale}`;
}

function withTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

function buildCsp(nonce: string | undefined): string {
  const api = QURAN.apiBase ? withTrailingSlash(QURAN.apiBase) : "";
  const connectSrc = [
    "'self'",
    "https://*.firebaseio.com",
    "wss://*.firebaseio.com",
    "https://firestore.googleapis.com",
    "https://firebase.googleapis.com",
    "https://firebaseinstallations.googleapis.com",
    "https://firebaseremoteconfig.googleapis.com",
    "https://firebaselogging.googleapis.com",
    "https://firebaselogging-pa.googleapis.com",
    "https://fcmregistrations.googleapis.com",
    "https://play.google.com",
    "https://www.google-analytics.com",
    "https://www.google.com",
  ];
  if (api) connectSrc.push(api);
  if (import.meta.env.DEV) {
    connectSrc.push("http://localhost:*", "ws://localhost:*", "wss://localhost:*");
  }
  const scriptSrc = ["'self'", "'wasm-unsafe-eval'"];
  if (nonce) scriptSrc.push(`'nonce-${nonce}'`);
  scriptSrc.push("https://www.gstatic.com", "https://www.googletagmanager.com");
  if (import.meta.env.DEV) {
    scriptSrc.push("'unsafe-eval'");
  }
  return [
    "default-src 'self'",
    `script-src ${scriptSrc.join(" ")}`,
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
  // SAFETY: getSetCookie exists at runtime on modern Headers implementations
  // but is missing from the TS lib target; the optional-prop intersection
  // mirrors exactly that runtime shape.
  const getter = (response.headers as Headers & { getSetCookie?: () => string[] }).getSetCookie;
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- feature-detecting the optional getSetCookie method on this platform's Headers; no I/O boundary to parse here
  return typeof getter === "function" && getter.call(response.headers).length > 0;
}

export function applyHeaders(
  response: Response,
  pathname: string,
  requestHasCookie = false,
  nonce?: string,
): void {
  response.headers.set("Content-Security-Policy", buildCsp(nonce));
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  response.headers.set("Strict-Transport-Security", "max-age=31536000; includeSubDomains");
  const translationPending = response.headers.get("x-eq-translation-pending");
  const privateMode = requestHasCookie || responseSetsCookie(response);
  const isRedirect = response.status >= 300 && response.status < 400;
  const isImmutableAsset =
    pathname.startsWith("/_app/immutable/") ||
    pathname.startsWith("/_quran/tanzil/") ||
    packPattern.test(pathname);
  if (privateMode && !isImmutableAsset) {
    response.headers.set("Cache-Control", "private, no-store");
  } else if (response.status >= 500 || hasNoStore(response) || translationPending) {
    response.headers.set("Cache-Control", "no-store");
  } else if (isRedirect) {
    // Redirect handlers own their cache policy (e.g. the bounded public TTL on
    // the deterministic legacy-locale 308); the default no-cache stays off.
  } else if (isImmutableAsset) {
    response.headers.set("Cache-Control", IMMUTABLE);
  } else {
    response.headers.set("Cache-Control", "no-cache");
  }
  const textVariant = pathname.endsWith(".md") || pathname.endsWith(".txt");
  if (textVariant) {
    response.headers.set("X-Robots-Tag", "noindex, follow");
  }
  if (textVariant || mdSiblingRequest(pathname) !== null || response.status === 404) {
    appendVaryAccept(response.headers);
  }
}

interface DocumentAttributes {
  readonly lang: string;
  readonly dir: UiDirection;
}

interface NonceHolder {
  nonce?: string;
}

function documentAttributes(uiLocale: UiLocale | null): DocumentAttributes {
  if (uiLocale) return { lang: uiLocale, dir: uiDirection(uiLocale) };
  return { lang: "en", dir: "ltr" };
}

function transformRootHtml(html: string, attributes: DocumentAttributes): string {
  return html
    .replace('lang="%lang%"', `lang="${attributes.lang}"`)
    .replace('dir="%dir%"', `dir="${attributes.dir}"`);
}

function tagInlineScripts(html: string, nonce: string): string {
  return html.replaceAll("<script>", `<script nonce="${nonce}">`);
}

/**
 * Scheme A redirect map: every legacy spelling resolves in ONE hop. Ordered
 * first-match rules; the numeric-alias and surah-local rewrites are composed
 * into the same 308 — no redirect chains. The table's outputs (unprefixed
 * paths, /surah, /ar/surah, /ar/<rest>) match no rule input, so no cycle.
 * 308 + bounded public TTL: targets are deterministic. If a target ever
 * becomes request-dependent, revert to 307 + no-store.
 */
function legacyPrefixRedirect(event: RequestEvent): Response | null {
  const { pathname } = event.url;
  let target: string | null = null;
  if (pathname === "/en") target = "/";
  else if (pathname.startsWith("/en/")) target = pathname.slice(3);
  else if (pathname === "/ar/app") target = "/ar/surah";
  else if (pathname.startsWith("/ar/app/")) target = `/ar${pathname.slice(7)}`;
  else if (pathname === "/app") target = "/surah";
  else if (pathname.startsWith("/app/")) target = pathname.slice(4);
  if (target === null) return null;
  // Collapse the legacy /en/app double prefix inside the same hop: an /en
  // strip can still leave the dead /app marker, and /en/app itself must land
  // on /surah like every other hub spelling.
  if (target === "/app") target = "/surah";
  else if (target.startsWith("/app/")) target = target.slice(4);

  // One-hop composition on the normalized target: numeric alias, then the D1
  // surah-local collapse. Both are pure path rewrites whose outputs match no
  // rule input (slugs are letter-initial, indexes are reserved words). The D1
  // ayah anchor rides along unless the inbound URL carries its own fragment.
  const search = building ? "" : event.url.search;
  const inboundHash = building ? "" : event.url.hash;
  const arPrefixed = target === "/ar" || target.startsWith("/ar/");
  const prefix = arPrefixed ? "/ar" : "";
  const rel = arPrefixed ? target.slice(3) : target;
  const aliased = numericAliasPath(rel);
  const local = aliased ? null : surahLocalRedirectTarget(rel);
  let location = prefix + (aliased ?? local?.path ?? rel);
  if (local && !inboundHash) location += local.fragment;
  const tail = `${search}${inboundHash}`;
  return new Response(null, {
    status: 308,
    headers: {
      location: `${location}${tail}`,
      "cache-control": "public, max-age=86400",
    },
  });
}

/**
 * Prefix view of a request: `/ar/**` keeps its locale; everything else is the
 * unprefixed en canonical. The redirect builders regex-gate themselves on the
 * returned rel, so a non-null return for every path is safe (cheap rejects).
 */
/** Prefix view of a request path: locale prefix ("" for the en canonical) + rel. */
interface ReaderRequestBase {
  prefix: string;
  rel: string;
}

function readerRequestBase(pathname: string): ReaderRequestBase {
  if (pathname === "/ar" || pathname.startsWith("/ar/")) {
    return { prefix: "/ar", rel: pathname.slice(3) };
  }
  return { prefix: "", rel: pathname };
}

/** D14 numeric chapter alias on a prefix-less rel: `/2` → `/al-baqarah`. */
function numericAliasPath(rel: string): string | null {
  const match = /^\/([1-9][0-9]*)$/u.exec(rel);
  if (!match) return null;
  const num = Number(match[1]);
  const surah = num >= 1 && num <= SURAH_COUNT ? QURAN_DATA.surahByNum(num) : undefined;
  return surah ? `/${surah.slug}` : null;
}

/**
 * Numeric chapter alias (D14): `/2` → `/al-baqarah`, `/ar/2` → `/ar/al-baqarah`.
 * Digits never collide with surah slugs (letter-initial) or the reserved range
 * segments. Out-of-range numbers fall through to the parse 404. Same bounded
 * public TTL as every other 308 row (plan §10: the TTL rides on every row).
 */
function numericChapterRedirect(event: RequestEvent): Response | null {
  const base = readerRequestBase(event.url.pathname);
  const slug = numericAliasPath(base.rel);
  if (!slug) return null;
  const tail = building ? "" : `${event.url.search}${event.url.hash}`;
  return new Response(null, {
    status: 308,
    headers: {
      location: `${base.prefix}${slug}${tail}`,
      "cache-control": "public, max-age=86400",
    },
  });
}

/**
 * Surah-local page URLs are gone (D1); every removed shape 308s to the surah
 * root, landing on the spread's first ayah when no explicit fragment travels
 * with the request.
 */
function surahLocalPageRedirect(event: RequestEvent): Response | null {
  const base = readerRequestBase(event.url.pathname);
  const target = surahLocalRedirectTarget(base.rel);
  if (!target) return null;
  const search = building ? "" : event.url.search;
  const inboundHash = building ? "" : event.url.hash;
  const fragment = inboundHash || target.fragment;
  return new Response(null, {
    status: 308,
    headers: { location: `${base.prefix}${target.path}${search}${fragment}` },
  });
}

function notFound(event: RequestEvent): Response {
  if (prefersMarkdown(event)) {
    return new Response(agentNotFoundMarkdown(), {
      status: 404,
      headers: { "content-type": "text/markdown; charset=utf-8", vary: "Accept" },
    });
  }
  return new Response("Not found", {
    status: 404,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}

function notAcceptable(accept: string): Response {
  return new Response(notAcceptableBody(accept), {
    status: 406,
    headers: {
      "cache-control": "no-store",
      "content-type": "text/plain; charset=utf-8",
      vary: "Accept",
    },
  });
}

function prefersMarkdown(event: RequestEvent): boolean {
  return (
    event.request.method === "GET" &&
    !event.isDataRequest &&
    !event.isSubRequest &&
    preferredType(event.request.headers.get("accept"), NEGOTIABLE_TYPES) === "text/markdown"
  );
}

async function resolveRequest(
  event: RequestEvent,
  resolve: Parameters<Handle>[0]["resolve"],
  uiLocale: UiLocale | null,
  readerRoute: ParsedReaderRoute | null,
  requestHasCookie: boolean,
): Promise<{ response: Response; nonce: string }> {
  const key = translationRouteCacheKey(readerRoute, uiLocale);
  const cacheable =
    event.request.method === "GET" &&
    !event.isDataRequest &&
    key !== null &&
    !requestHasCookie &&
    !event.url.pathname.endsWith(".md");
  const attributes = documentAttributes(uiLocale);
  const nonce = freshNonce();
  const resolveOpts = {
    transformPageChunk: ({ html }: { html: string }) => {
      const out = transformRootHtml(html, attributes);
      return building ? out : tagInlineScripts(out, nonce);
    },
  };

  if (cacheable) {
    const hit = await getCachedHtml(key);
    if (hit !== null) {
      const response = new Response(hit.replaceAll(NONCE_PLACEHOLDER, nonce), {
        headers: {
          "content-type": "text/html; charset=utf-8",
          "server-timing": 'quran_ssr_cache;desc="hit"',
          "x-easyquran-quran-cache": "hit",
        },
      });
      return { response, nonce };
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
        .then((html) =>
          setCachedHtml(key, html.replaceAll(`nonce="${nonce}"`, `nonce="${NONCE_PLACEHOLDER}"`)),
        )
        .catch(() => {});
    }
    response.headers.set("server-timing", 'quran_ssr_cache;desc="miss"');
    response.headers.set("x-easyquran-quran-cache", "miss");
    return { response, nonce };
  }

  return { response: await resolve(event, resolveOpts), nonce };
}

/**
 * The en-only marketing pages have no published /ar variant; a /ar-prefixed
 * request for them must NOT enter paraglide (it would resolve locale ar and
 * the page's unpublished-publication guard would 500). They render the en
 * chrome exactly like they did before the flip.
 */
// SAFETY: Object.keys over MARKETING_PUBLICATIONS (a `satisfies
// Readonly<Record<MarketingPageId, readonly UiLocale[]>>` literal) yields
// exactly its MarketingPageId keys; the cast restores what the Record key type
// already guarantees, and the locale membership test needs no cast at all.
const EN_ONLY_MARKETING_AR_PATHS: ReadonlySet<string> = new Set(
  (Object.keys(MARKETING_PUBLICATIONS) as MarketingPageId[])
    .filter((id) => {
      const locales: readonly string[] = MARKETING_PUBLICATIONS[id];
      return !locales.includes("ar");
    })
    .map((id) => `/ar${MARKETING_PATHS[id]}`),
);

/** The bounded app pages: live at the site root, with /ar twins via reroute. */
const PRODUCT_ROUTE_IDS: ReadonlySet<string> = new Set([
  "/(application)/search",
  "/(application)/settings",
  "/(application)/bookmarks",
  "/(application)/yours",
]);

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;
  const requestHasCookie = !!event.request.headers.get("cookie");
  const mdSibling = mdSiblingRequest(pathname);
  let response: Response | null = null;
  let nonce: string | undefined;
  let negotiated = false;

  if (
    mdSibling !== null &&
    event.request.method === "GET" &&
    !event.isDataRequest &&
    !event.isSubRequest
  ) {
    const accept = event.request.headers.get("accept");
    const chosen = preferredType(accept, NEGOTIABLE_TYPES);
    if (accept !== null && chosen === null) {
      response = notAcceptable(accept);
      negotiated = true;
    } else if (chosen === "text/markdown") {
      // A sibling miss must fall through to the 404 path, never surface as a
      // 500 — the fetch targets our own origin, but treat any transport error
      // the same as a non-ok response.
      const md = await event.fetch(mdSibling.mdPath.replace(/^\/ar(?=\/)/u, "")).catch(() => null);
      if (md?.ok) {
        response = new Response(await md.text(), {
          headers: {
            "content-type": "text/markdown; charset=utf-8",
            "x-robots-tag": "noindex, follow",
            vary: "Accept",
          },
        });
        negotiated = true;
      }
    }
  }

  if (!response) {
    response = legacyPrefixRedirect(event);
  }

  if (!response) {
    const readerLocale = localizedReaderLocale(pathname);
    // Scheme A gate: paraglide owns the localized marketing home and every
    // /ar/** application path. Unprefixed application paths skip it (baseLocale
    // fallback), exactly as /app/bookmarks did before the flip, and the
    // en-only marketing pages have no /ar publication to serve.
    const normalized = pathname.replace(/\/+$/u, "") || "/";
    const useI18n =
      pathname === "/" ||
      pathname === "/ar" ||
      (pathname.startsWith("/ar/") && !EN_ONLY_MARKETING_AR_PATHS.has(normalized));
    // Both redirect builders gate themselves on their rel regexes (numeric
    // alias, surah-local shapes), so attempt them for every request — cheap
    // null returns for everything else.
    const readerRedirect = numericChapterRedirect(event) ?? surahLocalPageRedirect(event);
    if (readerRedirect) {
      response = readerRedirect;
    } else if (
      readerLocale &&
      !parseReaderRoute(event.route.id, event.params) &&
      // Q1 default: /ar/search|settings|bookmarks|yours are live product-page
      // twins — valid localized routes with no reader-route descriptor.
      !PRODUCT_ROUTE_IDS.has(event.route.id ?? "")
    ) {
      response = notFound(event);
    } else if (useI18n) {
      const resolved: NonceHolder = {};
      response = await paraglideMiddleware(event.request, async ({ locale }) => {
        if (!isUiLocale(locale)) return notFound(event);
        // kit 3 made `event.request` readonly. The kit 2 code re-assigned
        // paraglide's re-localized Request here, but nothing downstream reads
        // `event.request.url` (only method + headers, which the localized
        // clone preserves verbatim), so the assignment is dropped, not replaced.
        const readerRoute = readerLocale ? parseReaderRoute(event.route.id, event.params) : null;
        const out = await resolveRequest(event, resolve, locale, readerRoute, requestHasCookie);
        resolved.nonce = out.nonce;
        return out.response;
      });
      if (resolved.nonce) nonce = resolved.nonce;
    } else {
      // Unprefixed application paths (baseLocale en) skip paraglide exactly as
      // the en canonical should — but they still resolve with the en UI locale
      // so translated-route HTML keeps its __ui-en disk-cache partition.
      const resolved = await resolveRequest(
        event,
        resolve,
        "en",
        parseReaderRoute(event.route.id, event.params),
        requestHasCookie,
      );
      response = resolved.response;
      nonce = resolved.nonce;
    }
  }

  if (
    response.status === 404 &&
    !(response.headers.get("content-type") ?? "").includes("text/markdown") &&
    prefersMarkdown(event)
  ) {
    response = new Response(agentNotFoundMarkdown(), {
      status: 404,
      headers: { "content-type": "text/markdown; charset=utf-8", vary: "Accept" },
    });
  }

  if (mdSibling !== null && !negotiated) {
    appendVaryAccept(response.headers);
    if ((response.headers.get("content-type") ?? "").includes("text/html")) {
      const link = `<${mdSibling.mdPath}>; rel="alternate"; type="text/markdown"`;
      const existingLink = response.headers.get("link");
      response.headers.set("Link", existingLink ? `${existingLink}, ${link}` : link);
    }
  }

  applyHeaders(response, pathname, requestHasCookie, nonce);
  return response;
};
