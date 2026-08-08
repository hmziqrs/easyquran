import { beforeEach, afterEach, describe, expect, it, vi } from "vite-plus/test";
import type { SourceCatalogueEntry } from "$lib/data/quran-types";

const { site, decode } = vi.hoisted(() => ({
  site: {
    apiBase: "https://api.test/quran",
    artifactBase: "https://art.test",
    scripts: [
      {
        id: "uthmani",
        sizeBytes: 1,
        downloadUrl: "https://art.test/scripts/uthmani.sqlite",
      },
    ],
  },
  decode: vi.fn(),
}));

vi.mock("$lib/config/site", () => ({ QURAN: site }));
vi.mock("$lib/quran/wire", () => ({ decodeSourcesPayload: decode }));

type CatalogueModule = typeof import("$lib/quran/catalogue");
let mod: CatalogueModule;

function jsonResponse(body: unknown, ok = true): Response {
  return new Response(JSON.stringify(body), {
    status: ok ? 200 : 500,
    headers: { "content-type": "application/json" },
  });
}

beforeEach(async () => {
  vi.resetModules();
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  site.apiBase = "https://api.test/quran";
  site.artifactBase = "https://art.test";
  decode.mockReset();
  mod = await import("$lib/quran/catalogue");
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("fetchSourceCatalogue", () => {
  it("returns an empty list when the response is not ok", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse([], false));
    const out = await mod.fetchSourceCatalogue();
    expect(out).toEqual([]);
    expect(decode).not.toHaveBeenCalled();
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("returns an empty list when the payload fails to decode", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ garbage: true }));
    decode.mockReturnValue(null);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toEqual([]);
  });

  it("rewrites a translation entry's download url against the artifact base", async () => {
    const entry: SourceCatalogueEntry = {
      kind: "translation",
      entry: {
        id: "sq.nahi",
        language: "Albanian",
        languageCode: "sq",
        direction: "ltr",
        name: "Efendi Nahi",
        translator: "Hasan Efendi Nahi",
        sizeBytes: 1175552,
        downloadUrl: "https://remote.test/old/sq.nahi.sqlite",
      },
    };
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ kind: "translation" });
    if (out[0].kind === "translation") {
      expect(out[0].entry.downloadUrl).toBe(
        "https://art.test/tanzil/translations/sqlite/sq.nahi.sqlite",
      );
    }
  });

  it("rewrites an arabic spec's download url from QURAN.scripts", async () => {
    const arabic: SourceCatalogueEntry = {
      kind: "arabic",
      spec: {
        id: "uthmani",
        sizeBytes: 1,
        downloadUrl: "https://remote.test/old/uthmani.sqlite",
      },
    };
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [arabic] } }));
    decode.mockReturnValue([arabic]);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toHaveLength(1);
    if (out[0].kind === "arabic") {
      expect(out[0].spec.downloadUrl).toBe("https://art.test/scripts/uthmani.sqlite");
    }
  });

  it("returns [] when the external signal is pre-aborted", async () => {
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockImplementation((_url, init) =>
        (init?.signal as AbortSignal | undefined)?.aborted
          ? Promise.reject(new DOMException("aborted", "AbortError"))
          : Promise.resolve(jsonResponse({ data: { sources: [] } })),
      );
    const ac = new AbortController();
    ac.abort();
    const out = await mod.fetchSourceCatalogue(ac.signal);
    expect(out).toEqual([]);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });
});

describe("resolveSourceCatalogue", () => {
  it("returns the baked catalogue without fetching when apiBase is empty", async () => {
    site.apiBase = "";
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({}));
    const out = await mod.resolveSourceCatalogue();
    expect(out.length).toBe(mod.bakedTranslationCatalogue().length);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("caches a successful result and skips the network on a subsequent call", async () => {
    const entry: SourceCatalogueEntry = {
      kind: "translation",
      entry: {
        id: "sq.nahi",
        language: "Albanian",
        languageCode: "sq",
        direction: "ltr",
        name: "Efendi Nahi",
        translator: null,
        sizeBytes: 1,
        downloadUrl: "https://art.test/tanzil/translations/sqlite/sq.nahi.sqlite",
      },
    };
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    await mod.resolveSourceCatalogue();
    await mod.resolveSourceCatalogue();
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("refetches once the TTL expires", async () => {
    const entry: SourceCatalogueEntry = {
      kind: "translation",
      entry: {
        id: "sq.nahi",
        language: "Albanian",
        languageCode: "sq",
        direction: "ltr",
        name: "Efendi Nahi",
        translator: null,
        sizeBytes: 1,
        downloadUrl: "https://art.test/tanzil/translations/sqlite/sq.nahi.sqlite",
      },
    };
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    await mod.resolveSourceCatalogue();
    await vi.advanceTimersByTimeAsync(300_001);
    await mod.resolveSourceCatalogue();
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it("deduplicates concurrent in-flight calls to a single fetch", async () => {
    const entry: SourceCatalogueEntry = {
      kind: "translation",
      entry: {
        id: "sq.nahi",
        language: "Albanian",
        languageCode: "sq",
        direction: "ltr",
        name: "Efendi Nahi",
        translator: null,
        sizeBytes: 1,
        downloadUrl: "https://art.test/tanzil/translations/sqlite/sq.nahi.sqlite",
      },
    };
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    await Promise.all([
      mod.resolveSourceCatalogue(),
      mod.resolveSourceCatalogue(),
      mod.resolveSourceCatalogue(),
    ]);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("returns [] without fetching when the signal is already aborted", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({}));
    const ac = new AbortController();
    ac.abort();
    const out = await mod.resolveSourceCatalogue(ac.signal);
    expect(out).toEqual([]);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("falls back to the baked catalogue when the fetch resolves to an empty list", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [] } }));
    decode.mockReturnValue([]);
    const out = await mod.resolveSourceCatalogue();
    expect(out.length).toBe(mod.bakedTranslationCatalogue().length);
  });
});

describe("translationCatalogue helper", () => {
  it("extracts only translation entries from a mixed list", () => {
    const entries: SourceCatalogueEntry[] = [
      {
        kind: "arabic",
        spec: { id: "uthmani", sizeBytes: 1, downloadUrl: "https://a/uthmani" },
      },
      {
        kind: "translation",
        entry: {
          id: "x",
          language: "X",
          languageCode: "x",
          direction: "ltr",
          name: "X",
          translator: null,
          sizeBytes: 1,
          downloadUrl: "https://a/x",
        },
      },
    ];
    const out = mod.translationCatalogue(entries);
    expect(out).toHaveLength(1);
    expect(out[0].id).toBe("x");
  });
});
