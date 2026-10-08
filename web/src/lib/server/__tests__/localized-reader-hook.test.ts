import type { RequestEvent } from "@sveltejs/kit";
import type { Handle } from "@sveltejs/kit/hooks";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const cache = vi.hoisted(() => ({
  html: new Map<string, string>(),
  get: vi.fn<(key: string) => Promise<string | null>>(),
  set: vi.fn<(key: string, html: string) => Promise<void>>(),
}));

vi.mock("#lib/config/site.js", () => ({ QURAN: { apiBase: "" } }));

vi.mock("#lib/server/quran-disk-cache.js", async (importOriginal) => {
  const original = await importOriginal<typeof import("#lib/server/quran-disk-cache.js")>();
  return {
    ...original,
    getCachedHtml: cache.get,
    setCachedHtml: cache.set,
  };
});

import { QURAN_DATA } from "#lib/server/quran-data.js";

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

describe("legacy prefix redirect map (scheme A 308 table)", () => {
  const resolve = (): Parameters<Handle>[0]["resolve"] => htmlResolve("must-not-render");

  it.each([
    // [inbound, location] — every legacy spelling resolves in ONE hop.
    ["/en", "/"],
    ["/en/", "/"],
    ["/en/app", "/surah"],
    ["/en/app/al-fatihah", "/al-fatihah"],
    ["/en/app/al-fatihah/t/en/sahih", "/al-fatihah/t/en/sahih"],
    ["/app", "/surah"],
    ["/app/al-fatihah", "/al-fatihah"],
    ["/app/juz/30", "/juz/30"],
    ["/ar/app", "/ar/surah"],
    ["/ar/app/al-fatihah", "/ar/al-fatihah"],
    ["/ar/app/page/13", "/ar/page/13"],
    // single-hop composition: numeric alias
    ["/en/app/2", "/al-baqarah"],
    ["/app/2", "/al-baqarah"],
    ["/ar/app/114", "/ar/an-nas"],
    // single-hop composition: removed surah-local shape (page one has no anchor)
    ["/app/al-fatihah/page/1", "/al-fatihah"],
    ["/en/app/al-fatihah/page/1", "/al-fatihah"],
    ["/ar/app/al-fatihah/page/1", "/ar/al-fatihah"],
  ])("308s %s → %s in one hop", async (inbound, location) => {
    const res = await handle({ event: requestEvent(inbound, null, {}), resolve: resolve() });
    expect(res.status).toBe(308);
    expect(res.headers.get("location")).toBe(location);
    expect(res.headers.get("cache-control")).toBe("public, max-age=86400");
  });

  it("rides the D1 ayah anchor on a composed page-N redirect (no inbound fragment)", async () => {
    const spread = QURAN_DATA.surahLocalPage(2, 2);
    if (!spread) throw new Error("missing al-baqarah local page 2");
    const res = await handle({
      event: requestEvent("/app/al-baqarah/page/2", null, {}),
      resolve: htmlResolve("must-not-render"),
    });
    expect(res.status).toBe(308);
    expect(res.headers.get("location")).toBe(`/al-baqarah#ayah-2-${spread.startAyah}`);

    // An inbound fragment wins over the computed anchor.
    const custom = await handle({
      event: requestEvent("/app/al-baqarah/page/2#custom", null, {}),
      resolve: htmlResolve("must-not-render"),
    });
    expect(custom.headers.get("location")).toBe("/al-baqarah#custom");
  });

  it("preserves query and hash on every legacy redirect", async () => {
    const res = await handle({
      event: requestEvent("/app/al-fatihah?view=reading#ayah-1-1", null, {}),
      resolve: resolve(),
    });
    expect(res.status).toBe(308);
    expect(res.headers.get("location")).toBe("/al-fatihah?view=reading#ayah-1-1");
  });

  it("keeps the bounded public TTL and never renders or caches on a legacy hit", async () => {
    const resolver = resolve();
    const event = requestEvent("/en/app/al-fatihah?view=reading", "/(application)/[surah]", {
      surah: "al-fatihah",
    });
    const response = await handle({ event, resolve: resolver });
    expect(response.status).toBe(308);
    expect(response.headers.get("location")).toBe("/al-fatihah?view=reading");
    expect(response.headers.get("cache-control")).toBe("public, max-age=86400");
    const csp = response.headers.get("content-security-policy") ?? "";
    const scriptSrc = csp.split("; ").find((directive) => directive.startsWith("script-src")) ?? "";
    expect(csp).toContain("default-src 'self'");
    expect(scriptSrc).toContain("'unsafe-eval'");
    expect(scriptSrc).not.toContain("'unsafe-inline'");
    expect(resolver).not.toHaveBeenCalled();
    expect(cache.get).not.toHaveBeenCalled();
  });

  it("numeric aliases live at the root with bounded range enforcement", async () => {
    const r = resolve();
    const first = await handle({ event: requestEvent("/2", null, {}), resolve: r });
    expect(first.status).toBe(308);
    expect(first.headers.get("location")).toBe("/al-baqarah");
    // The TTL rides on every 308 row, standalone numeric alias included.
    expect(first.headers.get("cache-control")).toBe("public, max-age=86400");

    const last = await handle({ event: requestEvent("/ar/114", null, {}), resolve: r });
    expect(last.headers.get("location")).toBe("/ar/an-nas");
    expect(last.headers.get("cache-control")).toBe("public, max-age=86400");

    // Out-of-range numbers 404 through the localized parse gate (ar is
    // prefix-detectable; unprefixed digits fall through to the [surah] 404).
    const over = await handle({ event: requestEvent("/ar/115", null, {}), resolve: r });
    expect(over.status).toBe(404);
    const zero = await handle({ event: requestEvent("/ar/0", null, {}), resolve: r });
    expect(zero.status).toBe(404);
    expect(r).not.toHaveBeenCalled();
  });
});

describe("server localized reader integration", () => {
  it("partitions translated HTML by bounded UI locale while keeping content language", async () => {
    const routeId = "/(application)/[surah]/t/[lang]/[translator]";
    const params = { surah: "al-fatihah", lang: "en", translator: "sahih" };
    // Scheme A: en is the unprefixed form, ar keeps the /ar prefix.
    const enResponse = await handle({
      event: requestEvent("/al-fatihah/t/en/sahih", routeId, params),
      resolve: htmlResolve("english-ui"),
    });
    const arResponse = await handle({
      event: requestEvent("/ar/al-fatihah/t/en/sahih", routeId, params),
      resolve: htmlResolve("arabic-ui"),
    });

    const keys = cache.set.mock.calls.map(([key]) => key);
    expect(keys).toHaveLength(2);
    expect(keys[0]).toContain("__en.sahih__surah__1__ui-en");
    expect(keys[1]).toContain("__en.sahih__surah__1__ui-ar");
    expect(await enResponse.text()).toContain('<html lang="en" dir="ltr">');
    expect(await arResponse.text()).toContain('<html lang="ar" dir="rtl">');
    expect(enResponse.headers.get("content-security-policy")).toContain("nonce-");
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
        "/ar/al-fatihah/t/en/sahih",
        "/(application)/[surah]/t/[lang]/[translator]",
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

  it("uses English document chrome for the unprefixed English UI over Arabic Quran content", async () => {
    const response = await handle({
      event: requestEvent("/al-fatihah", "/(application)/[surah]", {
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
        "/al-fatihah/t/ur/jalandhry",
        "/(application)/[surah]/t/[lang]/[translator]",
        { surah: "al-fatihah", lang: "ur", translator: "jalandhry" },
      ),
      resolve: htmlResolve(),
    });

    expect(await response.text()).toContain('<html lang="en" dir="ltr">');
  });

  it("rejects an invalid localized translated source before cache lookup or rendering", async () => {
    const event = requestEvent(
      "/ar/al-fatihah/t/en/not-in-catalogue",
      "/(application)/[surah]/t/[lang]/[translator]",
      { surah: "al-fatihah", lang: "en", translator: "not-in-catalogue" },
    );
    const resolve = htmlResolve();

    const response = await handle({ event, resolve });

    expect(response.status).toBe(404);
    expect(cache.get).not.toHaveBeenCalled();
    expect(cache.set).not.toHaveBeenCalled();
    expect(resolve).not.toHaveBeenCalled();
  });

  it("404s an unknown /ar reader slug through the parse gate", async () => {
    const event = requestEvent("/ar/not-a-surah", "/(application)/[surah]", {
      surah: "not-a-surah",
    });
    const resolve = htmlResolve();
    const response = await handle({ event, resolve });
    expect(response.status).toBe(404);
    expect(resolve).not.toHaveBeenCalled();
  });

  it("bypasses shared translated cache for cookie-bearing requests", async () => {
    const event = requestEvent(
      "/al-fatihah/t/en/sahih",
      "/(application)/[surah]/t/[lang]/[translator]",
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
