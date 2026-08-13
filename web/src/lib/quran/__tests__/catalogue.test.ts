import { beforeEach, afterEach, describe, expect, it, vi } from "vite-plus/test";
import type { QuranSourceId, SourceCatalogueEntry } from "$lib/data/quran-types";

const { site, decode, track, consentState } = vi.hoisted(() => ({
  site: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "/_quran/scripts/uthmani.sqlite" }],
  },
  decode: vi.fn(),
  track: vi.fn(),
  consentState: { analytics: true },
}));

vi.mock("$lib/config/site", () => ({ QURAN: site }));
vi.mock("$lib/quran/wire", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/quran/wire")>();
  return { ...actual, decodeSourcesPayload: decode };
});
vi.mock("$lib/firebase/analytics", () => ({ track }));
vi.mock("$lib/stores/consent.svelte", () => ({ consent: consentState }));

type CatalogueModule = typeof import("$lib/quran/catalogue");
let mod: CatalogueModule;

const TRANSLATION_R2_URL = "https://r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite";
const TRANSLATION_LOCAL_URL = "/_quran/tanzil/translations/sqlite/sq.nahi.sqlite";
const ARABIC_R2_URL = "https://r2.easyquran.fyi/scripts/uthmani.sqlite";
const ARABIC_LOCAL_URL = "/_quran/scripts/uthmani.sqlite";
const SQ_NAHI_SIZE = 1175552;

function jsonResponse(body: unknown, ok = true): Response {
  return new Response(JSON.stringify(body), {
    status: ok ? 200 : 500,
    headers: { "content-type": "application/json" },
  });
}

function translationEntry(
  overrides: Partial<{
    id: string;
    sizeBytes: number;
    downloadUrl: string;
  }> = {},
): SourceCatalogueEntry {
  return {
    kind: "translation",
    entry: {
      id: overrides.id ?? "sq.nahi",
      language: "Albanian",
      languageCode: "sq",
      direction: "ltr",
      name: "Efendi Nahi",
      translator: "Hasan Efendi Nahi",
      sizeBytes: overrides.sizeBytes ?? SQ_NAHI_SIZE,
      downloadUrl: overrides.downloadUrl ?? TRANSLATION_R2_URL,
    },
  };
}

function arabicEntry(
  overrides: Partial<{ id: string; sizeBytes: number; downloadUrl: string }> = {},
): SourceCatalogueEntry {
  return {
    kind: "arabic",
    spec: {
      id: (overrides.id ?? "uthmani") as QuranSourceId,
      sizeBytes: overrides.sizeBytes ?? 1,
      downloadUrl: overrides.downloadUrl ?? ARABIC_R2_URL,
    },
  };
}

beforeEach(async () => {
  vi.resetModules();
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  site.apiBase = "https://api.test/quran";
  site.artifactBase = "/_quran";
  site.scripts = [{ id: "uthmani", sizeBytes: 1, downloadUrl: ARABIC_LOCAL_URL }];
  decode.mockReset();
  track.mockReset();
  consentState.analytics = true;
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

  it("localizes a valid production R2 url to the baked same-origin path", async () => {
    const entry = translationEntry();
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toHaveLength(1);
    expect(out[0]).toMatchObject({ kind: "translation" });
    if (out[0]!.kind === "translation") {
      expect(out[0]!.entry.downloadUrl).toBe(TRANSLATION_LOCAL_URL);
      expect(out[0]!.entry.sizeBytes).toBe(SQ_NAHI_SIZE);
    }
  });

  it("localizes a registered arabic spec to the baked same-origin path", async () => {
    const arabic = arabicEntry();
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [arabic] } }));
    decode.mockReturnValue([arabic]);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toHaveLength(1);
    if (out[0]!.kind === "arabic") {
      expect(out[0]!.spec.downloadUrl).toBe(ARABIC_LOCAL_URL);
      expect(out[0]!.spec.sizeBytes).toBe(1);
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

  it("rejects the whole payload when one translation id is unknown", async () => {
    const valid = translationEntry();
    const unknown = translationEntry({ id: "xx.fake", downloadUrl: TRANSLATION_R2_URL });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { sources: [valid, unknown] } }),
    );
    decode.mockReturnValue([valid, unknown]);
    const out = await mod.fetchSourceCatalogue();
    expect(out).toEqual([]);
  });

  it("rejects the payload on a canonical-path mismatch", async () => {
    const entry = translationEntry({ downloadUrl: "https://r2.easyquran.fyi/evil/sq.nahi.sqlite" });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    expect(await mod.fetchSourceCatalogue()).toEqual([]);
  });

  it("rejects the payload on a size mismatch", async () => {
    const entry = translationEntry({ sizeBytes: SQ_NAHI_SIZE + 1 });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    expect(await mod.fetchSourceCatalogue()).toEqual([]);
  });

  it("rejects the payload on an http (non-https) origin", async () => {
    const entry = translationEntry({
      downloadUrl: "http://r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite",
    });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    expect(await mod.fetchSourceCatalogue()).toEqual([]);
  });

  it.each([
    ["credentials", "https://user:pass@r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite"],
    ["query", "https://r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite?x=1"],
    ["fragment", "https://r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite#frag"],
  ])("rejects the payload when the url carries %s", async (_label, downloadUrl) => {
    const entry = translationEntry({ downloadUrl });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    expect(await mod.fetchSourceCatalogue()).toEqual([]);
  });

  it("emits a single consent-gated telemetry event when the payload is rejected", async () => {
    vi.useRealTimers();
    const entry = translationEntry({ sizeBytes: SQ_NAHI_SIZE + 1 });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    consentState.analytics = true;
    await mod.fetchSourceCatalogue();
    await vi.waitFor(() => {
      expect(track).toHaveBeenCalledTimes(1);
      expect(track).toHaveBeenCalledWith("quran_artifact_rejected", { reason: "sources_payload" });
    });
  });

  it("emits a consent-gated telemetry event when the payload fails to decode", async () => {
    vi.useRealTimers();
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ garbage: true }));
    decode.mockReturnValue(null);
    consentState.analytics = true;
    const out = await mod.fetchSourceCatalogue();
    expect(out).toEqual([]);
    await vi.waitFor(() => {
      expect(track).toHaveBeenCalledTimes(1);
      expect(track).toHaveBeenCalledWith("quran_artifact_rejected", {
        reason: "sources_payload_malformed",
      });
    });
  });

  it("suppresses telemetry when consent is denied", async () => {
    const entry = translationEntry({ sizeBytes: SQ_NAHI_SIZE + 1 });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    consentState.analytics = false;
    await mod.fetchSourceCatalogue();
    await vi.advanceTimersByTimeAsync(0);
    expect(track).not.toHaveBeenCalled();
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
    const entry = translationEntry();
    const fetchSpy = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
    await mod.resolveSourceCatalogue();
    await mod.resolveSourceCatalogue();
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("refetches once the TTL expires", async () => {
    const entry = translationEntry();
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
    const entry = translationEntry();
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

  it("falls back to the baked catalogue when validation rejects the payload", async () => {
    const entry = translationEntry({ sizeBytes: SQ_NAHI_SIZE + 1 });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ data: { sources: [entry] } }));
    decode.mockReturnValue([entry]);
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
    expect(out[0]!.id).toBe("x");
  });
});
