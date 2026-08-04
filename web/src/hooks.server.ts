import type { Handle } from "@sveltejs/kit";
import { getCachedHtml, htmlCacheKey, setCachedHtml } from "$lib/server/quran-disk-cache";

const IMMUTABLE = "public, max-age=31536000, immutable";

const packPattern = /^\/offline\/pack\.[0-9a-f]+\.json$/u;

function isTranslationRoute(id: string | null): boolean {
  return id !== null && id.includes("/t/[lang]/[translator]");
}

function applyHeaders(response: Response, pathname: string): void {
  if (pathname.startsWith("/_app/immutable/") || packPattern.test(pathname)) {
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
  const cacheable =
    event.request.method === "GET" && isTranslationRoute(event.route.id);

  if (cacheable) {
    const key = htmlCacheKey(pathname);
    const hit = await getCachedHtml(key);
    if (hit !== null) {
      const response = new Response(hit, {
        headers: { "content-type": "text/html; charset=utf-8" },
      });
      applyHeaders(response, pathname);
      return response;
    }
    const response = await resolve(event);
    const contentType = response.headers.get("content-type") ?? "";
    if (response.status === 200 && contentType.includes("text/html")) {
      const clone = response.clone();
      void clone
        .text()
        .then((html) => setCachedHtml(key, html))
        .catch(() => {});
    }
    applyHeaders(response, pathname);
    return response;
  }

  const response = await resolve(event);
  applyHeaders(response, pathname);
  return response;
};
