import type { Handle, RequestEvent } from "@sveltejs/kit";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const cache = vi.hoisted(() => ({
  get: vi.fn<(key: string) => Promise<string | null>>(),
  set: vi.fn<(key: string, html: string) => Promise<void>>(),
}));

vi.mock("$lib/config/site", () => ({ QURAN: { apiBase: "" } }));

vi.mock("$lib/data/translations.json", async (importOriginal) => {
  const original = await importOriginal<{ default: unknown[] }>();
  return {
    default: [
      ...original.default,
      [
        "en.sahih.int",
        "English",
        "en",
        "ltr",
        "Dotted translator fixture",
        "Sahih International",
        "sqlite/en.sahih.int.sqlite",
        1,
      ],
    ],
  };
});

vi.mock("$lib/server/quran-disk-cache", async (importOriginal) => {
  const original = await importOriginal<typeof import("$lib/server/quran-disk-cache")>();
  return { ...original, getCachedHtml: cache.get, setCachedHtml: cache.set };
});

import { parseReaderPath, parseReaderRoute } from "$lib/server/reader-route";

import { reroute } from "../../../hooks";
import { handle } from "../../../hooks.server";

const ROUTE_ID = "/(application)/app/[surah]/t/[lang]/[translator]";
const PARAMS = { surah: "al-fatihah", lang: "en", translator: "sahih.int" };

function event(): RequestEvent {
  const url = new URL("https://easyquran.fyi/en/app/al-fatihah/t/en/sahih.int");
  // SAFETY: test double providing every RequestEvent member the handle() path reads; cookies and locals are inert placeholders in this test.
  return {
    // SAFETY: handle() reads no cookies in this test; the empty object satisfies the accessor shape RequestEvent declares.
    cookies: {} as RequestEvent["cookies"],
    fetch,
    getClientAddress: () => "127.0.0.1",
    isDataRequest: false,
    isRemoteRequest: false,
    isSubRequest: false,
    locals: {},
    params: PARAMS,
    platform: undefined,
    request: new Request(url),
    route: { id: ROUTE_ID },
    setHeaders: () => {},
    // SAFETY: tracing is kit-internal telemetry; the localized handle() path under test never reads it.
    tracing: { enabled: false } as RequestEvent["tracing"],
    url,
  } as RequestEvent;
}

beforeEach(() => {
  cache.get.mockReset().mockResolvedValue(null);
  cache.set.mockReset().mockResolvedValue(undefined);
});

describe("dotted translator route segments", () => {
  it("reroutes the public URL without splitting the translator segment", async () => {
    const url = new URL("https://easyquran.fyi/ar/app/al-fatihah/t/en/sahih.int");
    expect(await reroute({ url, fetch })).toBe("/app/al-fatihah/t/en/sahih.int");
  });

  it("parses a baked dotted source from path and resolved route params", () => {
    expect(parseReaderPath("/app/al-fatihah/t/en/sahih.int")).toMatchObject({
      type: "translation",
      sourceId: "en.sahih.int",
      cacheKind: "surah",
      index: 1,
    });
    expect(parseReaderRoute(ROUTE_ID, PARAMS)).toMatchObject({
      type: "translation",
      sourceId: "en.sahih.int",
    });
    expect(parseReaderPath("/app/al-fatihah/t/en/not.baked")).toBeNull();
  });

  it("uses full dotted source ID in bounded localized HTML cache key", async () => {
    const resolve = vi.fn<Parameters<Handle>[0]["resolve"]>(async (_event, options) => {
      const source = '<html lang="%lang%" dir="%dir%"><body>dotted</body></html>';
      const html = options?.transformPageChunk
        ? await options.transformPageChunk({ html: source, done: true })
        : source;
      return new Response(html, { headers: { "content-type": "text/html" } });
    });

    const response = await handle({ event: event(), resolve });

    expect(response.status).toBe(200);
    expect(cache.get).toHaveBeenCalledTimes(1);
    const key = cache.get.mock.calls[0]![0];
    expect(key).toContain("__en.sahih.int__surah__1__1__ui-en");
    expect(cache.set).toHaveBeenCalledWith(key, expect.stringContaining("dotted"));
  });
});
