import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { startMock, onStatusMock, onProgressMock, disposeMock, loadQuranDataMock } = vi.hoisted(
  () => ({
    startMock: vi.fn(),
    onStatusMock: vi.fn(),
    onProgressMock: vi.fn(),
    disposeMock: vi.fn(),
    loadQuranDataMock: vi.fn(),
  }),
);

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$lib/config/site", () => ({
  QURAN: {
    apiBase: "https://api.test/quran",
    artifactBase: "/_quran",
    scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "/_quran/uthmani.sqlite" }],
  },
}));
vi.mock("$lib/quran/worker-client", () => ({
  quranWorker: {
    start: startMock,
    onStatus: onStatusMock,
    onProgress: onProgressMock,
    dispose: disposeMock,
  },
}));
vi.mock("$lib/data/quran-data-client", () => ({ loadQuranData: loadQuranDataMock }));

const COORDS = Object.freeze({ rowCount: 6236, surahs: [1], pages: [1], juzs: [1] });

async function flush(): Promise<void> {
  for (let i = 0; i < 12; i++) await Promise.resolve();
}

describe("bootOfflineEngine", () => {
  beforeEach(() => {
    vi.resetModules();
    startMock.mockReset().mockResolvedValue(undefined);
    onStatusMock.mockReset().mockReturnValue(() => {});
    onProgressMock.mockReset().mockReturnValue(() => {});
    disposeMock.mockReset();
    loadQuranDataMock.mockReset().mockResolvedValue({ coordinates: COORDS });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts from baked manifest and catalogue without metadata requests", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const { bootOfflineEngine } = await import("$lib/quran/offline");
    const { quran } = await import("$lib/stores/quran.svelte");

    bootOfflineEngine();
    await flush();

    expect(startMock).toHaveBeenCalledTimes(1);
    // SAFETY: startMock replaces quranWorker.start(manifest, coordinates, catalogue); this test first proves one call exists, then reads that fixed three-argument contract.
    const [manifest, coordinates, catalogue] = startMock.mock.calls[0] as [
      { scripts: unknown[] },
      unknown,
      unknown[],
    ];
    expect(manifest.scripts).toHaveLength(1);
    expect(coordinates).toBe(COORDS);
    expect(catalogue.length).toBeGreaterThan(100);
    expect(quran.status).not.toBe("error");
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("disposes worker subscriptions and worker instance", async () => {
    const detachStatus = vi.fn();
    const detachProgress = vi.fn();
    onStatusMock.mockReturnValue(detachStatus);
    onProgressMock.mockReturnValue(detachProgress);
    const { bootOfflineEngine } = await import("$lib/quran/offline");

    const teardown = bootOfflineEngine();
    teardown();

    expect(detachStatus).toHaveBeenCalledTimes(1);
    expect(detachProgress).toHaveBeenCalledTimes(1);
    expect(disposeMock).toHaveBeenCalledTimes(1);
  });
});
