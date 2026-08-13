import type { Handle, RequestEvent } from "@sveltejs/kit";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const cache = vi.hoisted(() => ({
  html: new Map<string, string>(),
  get: vi.fn<(key: string) => Promise<string | null>>(),
  set: vi.fn<(key: string, html: string) => Promise<void>>(),
}));

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "" } }));

vi.mock("$lib/server/quran-disk-cache", async (importOriginal) => {
  const original = await importOriginal<typeof import("$lib/server/quran-disk-cache")>();
  return {
    ...original,
    getCachedHtml: cache.get,
    setCachedHtml: cache.set,
  };
});

import { handle } from "../../../hooks.server";

const ORIGIN = "https://easyquran.fyi";

function requestEvent(
  pathname: string,
  routeId: string | null,
  params: Record<string, string> = {},
  headers?: HeadersInit,
): RequestEvent {
  const url = new URL(pathname, ORIGIN);
  const request = new Request(url);
  for (const [name, value] of new Headers(headers)) request.headers.set(name, value);
  // SAFETY: test double providing every RequestEvent member the handle() path reads; cookies and locals are inert placeholders in this test.
  return {
    // SAFETY: handle() reads no cookies in these tests; the empty object satisfies the accessor shape RequestEvent declares.
    cookies: {} as RequestEvent["cookies"],
    fetch,
    getClientAddress: () => "127.0.0.1",
    isDataRequest: false,
    isRemoteRequest: false,
    isSubRequest: false,
    locals: {},
    params,
    platform: undefined,
    request,
    route: { id: routeId },
    setHeaders: () => {},
    // SAFETY: tracing is kit-internal telemetry; the localized handle() path under test never reads it.
    tracing: { enabled: false } as RequestEvent["tracing"],
    url,
  } as RequestEvent;
}

function htmlResolve(body = "rendered"): Parameters<Handle>[0]["resolve"] {
  return vi.fn(async (_event, options) => {
    const source = `<html lang="%lang%" dir="%dir%"><body>${body}</body></html>`;
    const html = options?.transformPageChunk
      ? await options.transformPageChunk({ html: source, done: true })
      : source;
    return new Response(html, { headers: { "content-type": "text/html; charset=utf-8" } });
  });
}

beforeEach(() => {
  cache.html.clear();
  cache.get.mockReset().mockImplementation(async (key) => cache.html.get(key) ?? null);
  cache.set.mockReset().mockImplementation(async (key, html) => {
    cache.html.set(key, html);
  });
});

describe("server localized reader integration", () => {
  it("redirects a valid legacy route to localized English and preserves query", async () => {
    const event = requestEvent("/app/al-fatihah?view=reading", "/(application)/app/[surah]", {
      surah: "al-fatihah",
    });
    const resolve = htmlResolve();

    const response = await handle({ event, resolve });

    expect(response.status).toBe(307);
    expect(response.headers.get("location")).toBe("/en/app/al-fatihah?view=reading");
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("content-security-policy")).toContain("default-src 'self'");
    expect(resolve).not.toHaveBeenCalled();
    expect(cache.get).not.toHaveBeenCalled();
  });

  it("partitions translated HTML by bounded UI locale while keeping content language", async () => {
    const routeId = "/(application)/app/[surah]/t/[lang]/[translator]";
    const params = { surah: "al-fatihah", lang: "en", translator: "sahih" };
    const enResponse = await handle({
      event: requestEvent("/en/app/al-fatihah/t/en/sahih", routeId, params),
      resolve: htmlResolve("english-ui"),
    });
    const arResponse = await handle({
      event: requestEvent("/ar/app/al-fatihah/t/en/sahih", routeId, params),
      resolve: htmlResolve("arabic-ui"),
    });

    const keys = cache.set.mock.calls.map(([key]) => key);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toContain("__en.sahih__surah__1__1__ui-en");
    expect(keys[1]).toContain("__en.sahih__surah__1__1__ui-ar");
    expect(await enResponse.text()).toContain('<html lang="en" dir="ltr">');
    expect(await arResponse.text()).toContain('<html lang="ar" dir="rtl">');
    expect(enResponse.headers.get("x-easyquran-quran-cache")).toBe("miss");
    expect(arResponse.headers.get("x-easyquran-quran-cache")).toBe("miss");
  });

  it("serves a warm translated hit only from the matching localized cache partition", async () => {
    cache.get.mockImplementation(async (key) =>
      key.endsWith("__ui-ar") ? '<html lang="en" dir="ltr"><body>cached-ar-ui</body></html>' : null,
    );
    const resolve = htmlResolve("must-not-render");

    const response = await handle({
      event: requestEvent(
        "/ar/app/al-fatihah/t/en/sahih",
        "/(application)/app/[surah]/t/[lang]/[translator]",
        { surah: "al-fatihah", lang: "en", translator: "sahih" },
      ),
      resolve,
    });

    expect(await response.text()).toContain("cached-ar-ui");
    expect(response.headers.get("x-easyquran-quran-cache")).toBe("hit");
    expect(response.headers.get("server-timing")).toContain("hit");
    expect(response.headers.get("content-security-policy")).toContain("default-src 'self'");
    expect(cache.get.mock.calls[0]![0]).toContain("__ui-ar");
    expect(cache.set).not.toHaveBeenCalled();
    expect(resolve).not.toHaveBeenCalled();
  });

  it("uses English document chrome for an English UI over Arabic Quran content", async () => {
    const response = await handle({
      event: requestEvent("/en/app/al-fatihah", "/(application)/app/[surah]", {
        surah: "al-fatihah",
      }),
      resolve: htmlResolve(),
    });

    const html = await response.text();
    expect(html).toContain('<html lang="en" dir="ltr">');
    expect(html).not.toContain("%lang%");
    expect(html).not.toContain("%dir%");
    expect(cache.get).not.toHaveBeenCalled();
  });

  it("uses English document chrome for an English UI over RTL translation content", async () => {
    const response = await handle({
      event: requestEvent(
        "/en/app/al-fatihah/t/ur/jalandhry",
        "/(application)/app/[surah]/t/[lang]/[translator]",
        { surah: "al-fatihah", lang: "ur", translator: "jalandhry" },
      ),
      resolve: htmlResolve(),
    });

    expect(await response.text()).toContain('<html lang="en" dir="ltr">');
  });

  it("never lets explicit translated page one hit or write the root cache entry", async () => {
    cache.html.set("warm-root", "<html>warm root</html>");
    cache.get.mockResolvedValue("<html>warm root</html>");
    const routeId = "/(application)/app/[surah]/t/[lang]/[translator]/page/[localPage]";
    const event = requestEvent("/en/app/al-fatihah/t/en/sahih/page/1?view=reading", routeId, {
      surah: "al-fatihah",
      lang: "en",
      translator: "sahih",
      localPage: "1",
    });
    const resolve = htmlResolve("must-not-render");

    const response = await handle({ event, resolve });

    expect(response.status).toBe(308);
    expect(response.headers.get("location")).toBe("/en/app/al-fatihah/t/en/sahih?view=reading");
    expect(cache.get).not.toHaveBeenCalled();
    expect(cache.set).not.toHaveBeenCalled();
    expect(resolve).not.toHaveBeenCalled();
  });

  it("canonicalizes explicit Arabic page one before rendering", async () => {
    const resolve = htmlResolve("must-not-render");
    const response = await handle({
      event: requestEvent(
        "/ar/app/al-fatihah/page/1?view=reading",
        "/(application)/app/[surah]/page/[localPage]",
        { surah: "al-fatihah", localPage: "1" },
      ),
      resolve,
    });

    expect(response.status).toBe(308);
    expect(response.headers.get("location")).toBe("/ar/app/al-fatihah?view=reading");
    expect(resolve).not.toHaveBeenCalled();
  });

  it("rejects an invalid translated source before cache lookup or rendering", async () => {
    const event = requestEvent(
      "/ar/app/al-fatihah/t/en/not-in-catalogue",
      "/(application)/app/[surah]/t/[lang]/[translator]",
      { surah: "al-fatihah", lang: "en", translator: "not-in-catalogue" },
    );
    const resolve = htmlResolve();

    const response = await handle({ event, resolve });

    expect(response.status).toBe(404);
    expect(cache.get).not.toHaveBeenCalled();
    expect(cache.set).not.toHaveBeenCalled();
    expect(resolve).not.toHaveBeenCalled();
  });

  it("bypasses shared translated cache for cookie-bearing requests", async () => {
    const event = requestEvent(
      "/en/app/al-fatihah/t/en/sahih",
      "/(application)/app/[surah]/t/[lang]/[translator]",
      { surah: "al-fatihah", lang: "en", translator: "sahih" },
      { cookie: "session=private" },
    );
    expect(event.request.headers.get("cookie")).toBe("session=private");
    const response = await handle({
      event,
      resolve: htmlResolve(),
    });

    expect(cache.get).not.toHaveBeenCalled();
    expect(cache.set).not.toHaveBeenCalled();
    expect(response.headers.get("cache-control")).toBe("private, no-store");
  });
});
