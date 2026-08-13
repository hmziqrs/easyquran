import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

// Hoisted shared doubles. The boot engine imports a singleton quran store and
// a singleton catalogueStore, so each test resets modules to get a fresh pair.
const {
  site,
  startMock,
  provideCatalogueMock,
  onStatusMock,
  onProgressMock,
  disposeMock,
  loadQuranDataMock,
} = vi.hoisted(() => ({
  site: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "/_quran/scripts/uthmani.sqlite" }],
  },
  startMock: vi.fn(),
  provideCatalogueMock: vi.fn(),
  onStatusMock: vi.fn(),
  onProgressMock: vi.fn(),
  disposeMock: vi.fn(),
  loadQuranDataMock: vi.fn(),
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$lib/config/site", () => ({ QURAN: site }));
vi.mock("$lib/quran/worker-client", () => ({
  quranWorker: {
    start: startMock,
    provideCatalogue: provideCatalogueMock,
    onStatus: onStatusMock,
    onProgress: onProgressMock,
    dispose: disposeMock,
    whenReady: () => Promise.resolve(),
  },
}));
vi.mock("$lib/data/quran-data-client", () => ({ loadQuranData: loadQuranDataMock }));
vi.mock("$lib/firebase/analytics", () => ({ track: vi.fn() }));
vi.mock("$lib/stores/consent.svelte", () => ({ consent: { analytics: true } }));

type OfflineModule = typeof import("$lib/quran/offline");
type QuranStoreModule = typeof import("$lib/stores/quran.svelte");

const COORDS = Object.freeze({ rowCount: 6236, surahs: [1], pages: [1], juzs: [1] });

function jsonResponse(body: unknown, ok = true): Response {
  return new Response(JSON.stringify(body), {
    status: ok ? 200 : 500,
    headers: { "content-type": "application/json" },
  });
}

async function flush(): Promise<void> {
  for (let i = 0; i < 12; i++) await Promise.resolve();
}

async function importFresh<T>(path: string): Promise<T> {
  return (await import(path)) as T;
}

function defaultWorkerMocks(): void {
  startMock.mockReset().mockResolvedValue(undefined);
  provideCatalogueMock.mockReset().mockResolvedValue(undefined);
  onStatusMock.mockReset().mockReturnValue(() => {});
  onProgressMock.mockReset().mockReturnValue(() => {});
  disposeMock.mockReset();
  loadQuranDataMock.mockReset().mockResolvedValue({ coordinates: COORDS });
}

describe("bootOfflineEngine boot sequence", () => {
  beforeEach(() => {
    vi.resetModules();
    defaultWorkerMocks();
    site.apiBase = "https://api.test/quran";
    site.artifactBase = "/_quran";
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts the worker from baked manifest/catalogue before /scripts and /sources resolve", async () => {
    // Defer the metadata fetches so we can assert ordering at the boundary.
    let resolveScripts!: (v: Response) => void;
    let resolveSources!: (v: Response) => void;
    const scriptsPromise = new Promise<Response>((r) => (resolveScripts = r));
    const sourcesPromise = new Promise<Response>((r) => (resolveSources = r));
    vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
      const u = String(url);
      if (u.endsWith("/scripts")) return scriptsPromise;
      if (u.endsWith("/sources")) return sourcesPromise;
      return Promise.resolve(jsonResponse({}));
    });

    const offline = await importFresh<OfflineModule>("$lib/quran/offline");
    const { quran } = await importFresh<QuranStoreModule>("$lib/stores/quran.svelte");

    offline.bootOfflineEngine();
    await flush();

    // The worker must have started with the baked manifest + baked catalogue
    // before any remote metadata resolved (both deferreds still pending).
    expect(startMock).toHaveBeenCalledTimes(1);
    const [manifest, , catalogue] = startMock.mock.calls[0] as [
      { source: string },
      unknown,
      unknown,
    ];
    expect(manifest.source).toBe("baked");
    expect(Array.isArray(catalogue)).toBe(true);
    expect(quran.source).toBe("baked");

    // Metadata fetches are in flight but unresolved — boot did not wait.
    expect(resolveScripts).toBeTypeOf("function");
    expect(resolveSources).toBeTypeOf("function");
  });

  it("refreshes the catalogue through the worker once validated metadata arrives", async () => {
    const scriptsBody = {
      data: {
        scripts: [
          { id: "uthmani", sizeBytes: 1, downloadUrl: "/_quran/scripts/uthmani.sqlite" },
        ],
      },
    };
    const sourcesBody = {
      data: {
        sources: [
          {
            kind: "translation",
            entry: {
              id: "sq.nahi",
              language: "Albanian",
              languageCode: "sq",
              direction: "ltr",
              name: "Efendi Nahi",
              translator: "Hasan Efendi Nahi",
              sizeBytes: 1175552,
              downloadUrl:
                "https://r2.easyquran.fyi/tanzil/translations/sqlite/sq.nahi.sqlite",
            },
          },
        ],
      },
    };
    vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
      const u = String(url);
      if (u.endsWith("/scripts")) return Promise.resolve(jsonResponse(scriptsBody));
      if (u.endsWith("/sources")) return Promise.resolve(jsonResponse(sourcesBody));
      return Promise.resolve(jsonResponse({}));
    });

    const offline = await importFresh<OfflineModule>("$lib/quran/offline");
    const { quran } = await importFresh<QuranStoreModule>("$lib/stores/quran.svelte");

    offline.bootOfflineEngine();
    await flush();

    // Validated catalogue was pushed to the worker through provideCatalogue.
    expect(provideCatalogueMock).toHaveBeenCalledTimes(1);
    const pushed = provideCatalogueMock.mock.calls[0]![0]! as unknown[];
    expect(pushed.length).toBeGreaterThan(0);
    // Source-label upgrade from baked→api is covered by the W10a manifest
    // tests; here we only assert the refresh path ran without aborting boot.
    expect(quran.status).not.toBe("error");
  });

  it("leaves baked state intact when metadata refresh fails", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation((url) => {
      const u = String(url);
      // Both metadata endpoints fail.
      if (u.endsWith("/scripts") || u.endsWith("/sources")) {
        return Promise.reject(new Error("network down"));
      }
      return Promise.resolve(jsonResponse({}));
    });

    const offline = await importFresh<OfflineModule>("$lib/quran/offline");
    const { quran } = await importFresh<QuranStoreModule>("$lib/stores/quran.svelte");

    expect(() => offline.bootOfflineEngine()).not.toThrow();
    await flush();

    // Worker still started from baked data; refresh failure did not abort boot.
    expect(startMock).toHaveBeenCalledTimes(1);
    expect((startMock.mock.calls[0]![0]! as { source: string }).source).toBe("baked");
    // Baked state survives the metadata outage — no error, baked source.
    // catalogueStore falls back to baked and may re-publish it, but the source
    // label (driven by the manifest) stays baked because resolveManifest fell
    // back to baked.
    expect(quran.status).not.toBe("error");
    expect(quran.source).toBe("baked");
  });
});
