import { beforeEach, afterEach, describe, expect, it, vi } from "vite-plus/test";

// Mirror of catalogue.test.ts for resolveManifest: remote manifest data may
// describe Arabic script availability but can never select Quran bytes, size,
// or a delivery origin. Every production download spec is reconstructed from
// baked id/size/path fields.
const { site, decode, track, consentState } = vi.hoisted(() => ({
  site: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [
      {
        id: "uthmani",
        sizeBytes: 2885632,
        downloadUrl: "/_quran/tanzil/arabic/quran-uthmani.sqlite",
      },
    ],
  },
  decode: vi.fn(),
  track: vi.fn(),
  consentState: { analytics: true },
}));

vi.mock("$lib/config/site", () => ({ QURAN: site }));
vi.mock("$lib/quran/wire", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/quran/wire")>();
  return { ...actual, decodeScriptsPayload: decode };
});
vi.mock("$lib/firebase/analytics", () => ({ track }));
vi.mock("$lib/stores/consent.svelte", () => ({ consent: consentState }));

type ManifestModule = typeof import("$lib/quran/manifest");
let mod: ManifestModule;

const ARABIC_R2_URL = "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite";
const ARABIC_LOCAL_URL = "/_quran/tanzil/arabic/quran-uthmani.sqlite";
const ARABIC_SIZE = 2885632;

function scriptSpec(overrides: Partial<{ id: string; sizeBytes: number; downloadUrl: string }> = {}) {
  return {
    id: overrides.id ?? "uthmani",
    sizeBytes: overrides.sizeBytes ?? ARABIC_SIZE,
    downloadUrl: overrides.downloadUrl ?? ARABIC_R2_URL,
  };
}

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
  site.artifactBase = "/_quran";
  site.scripts = [{ id: "uthmani", sizeBytes: ARABIC_SIZE, downloadUrl: ARABIC_LOCAL_URL }];
  decode.mockReset();
  track.mockReset();
  consentState.analytics = true;
  mod = await import("$lib/quran/manifest");
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("resolveManifest API -> baked fallback contract", () => {
  it("returns baked without fetching when apiBase is empty", async () => {
    site.apiBase = "";
    const fetchSpy = vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({}));
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("localizes a valid production R2 url to the baked same-origin path", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec()] } }),
    );
    decode.mockReturnValue([scriptSpec()]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("api");
    expect(out.scripts).toHaveLength(1);
    // Delivery URL + size reconstructed entirely from baked fields.
    expect(out.scripts[0]!.downloadUrl).toBe(ARABIC_LOCAL_URL);
    expect(out.scripts[0]!.sizeBytes).toBe(ARABIC_SIZE);
  });

  it("falls back to baked when the size mismatches", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })] } }),
    );
    decode.mockReturnValue([scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it("falls back to baked on a canonical R2 path mismatch", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({
        data: {
          scripts: [
            scriptSpec({ downloadUrl: "https://r2.easyquran.fyi/evil/uthmani.sqlite" }),
          ],
        },
      }),
    );
    decode.mockReturnValue([scriptSpec({ downloadUrl: "https://r2.easyquran.fyi/evil/uthmani.sqlite" })]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it("falls back to baked on an unknown script id", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec({ id: "xx.fake" })] } }),
    );
    decode.mockReturnValue([scriptSpec({ id: "xx.fake" })]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it("falls back to baked on an http (non-https) origin", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({
        data: {
          scripts: [
            scriptSpec({ downloadUrl: "http://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite" }),
          ],
        },
      }),
    );
    decode.mockReturnValue([
      scriptSpec({ downloadUrl: "http://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite" }),
    ]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it.each([
    ["credentials", "https://user:pass@r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite"],
    ["query", "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite?x=1"],
    ["fragment", "https://r2.easyquran.fyi/tanzil/arabic/quran-uthmani.sqlite#frag"],
  ])("falls back to baked when the url carries %s", async (_label, downloadUrl) => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec({ downloadUrl })] } }),
    );
    decode.mockReturnValue([scriptSpec({ downloadUrl })]);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it("falls back to baked when the payload is null/undecodable", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({ garbage: true }));
    decode.mockReturnValue(null);
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
  });

  it("falls back to baked when the response is not ok", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse({}, false));
    const out = await mod.resolveManifest();
    expect(out.source).toBe("baked");
    expect(decode).not.toHaveBeenCalled();
  });

  it("emits a single consent-gated telemetry event on validation rejection", async () => {
    vi.useRealTimers();
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })] } }),
    );
    decode.mockReturnValue([scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })]);
    consentState.analytics = true;
    await mod.resolveManifest();
    await vi.waitFor(() => {
      expect(track).toHaveBeenCalledTimes(1);
      expect(track).toHaveBeenCalledWith("quran_artifact_rejected", { reason: "scripts_payload" });
    });
  });

  it("suppresses telemetry when consent is denied", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({ data: { scripts: [scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })] } }),
    );
    decode.mockReturnValue([scriptSpec({ sizeBytes: ARABIC_SIZE + 1 })]);
    consentState.analytics = false;
    await mod.resolveManifest();
    await vi.advanceTimersByTimeAsync(0);
    expect(track).not.toHaveBeenCalled();
  });
});
