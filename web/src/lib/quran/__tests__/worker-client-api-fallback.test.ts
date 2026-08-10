import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { ResolvedManifest } from "$lib/quran/manifest";
import type { WorkerOutbound, WorkerRequest } from "$lib/quran/protocol";
import { LOCAL_BOOT_BUDGET_MS } from "$lib/quran/fetch";
import { QURAN_DATA } from "$lib/server/quran-data";

vi.mock("$lib/config/site", () => ({
  QURAN: { apiBase: "https://api.test/quran" },
}));

import { quranWorker } from "$lib/quran/worker-client";

const SURAH1 = {
  data: {
    sourceId: "uthmani",
    script: "uthmani",
    verses: ["ٱلْحَمْدُ", "لِلَّهِ"],
    normalization: {
      surah: 1,
      sourceId: "uthmani",
      script: "uthmani",
      sourceProfile: "tanzil-uthmani-581cc540",
      packaging: "numbered-ayah",
      openerKind: "verse",
      openerText: "ٱلْحَمْدُ",
      openerEndScalar: 0,
      bodyStartScalar: 0,
    },
  },
};

function okJson(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

type Listener = (e: unknown) => void;

class FakeWorker {
  static last: FakeWorker | null = null;
  readonly listeners = new Map<string, Set<Listener>>();
  readonly posted: WorkerRequest[] = [];
  terminated = false;

  constructor() {
    FakeWorker.last = this;
  }
  addEventListener(type: string, fn: Listener): void {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(fn);
  }
  removeEventListener(type: string, fn: Listener): void {
    this.listeners.get(type)?.delete(fn);
  }
  postMessage(msg: WorkerRequest): void {
    this.posted.push(msg);
  }
  terminate(): void {
    this.terminated = true;
  }
  emit(type: "message", data: WorkerOutbound): void;
  emit(type: "error" | "messageerror", payload: unknown): void;
  emit(type: string, payload: unknown): void {
    const evt = type === "message" ? { data: payload } : payload;
    this.listeners.get(type)?.forEach((fn) => fn(evt));
  }
}

const MANIFEST: ResolvedManifest = {
  scripts: [{ id: "uthmani", sizeBytes: 1, downloadUrl: "https://x/uthmani" }],
  source: "baked",
};

function mockFetchSurah(): ReturnType<typeof vi.spyOn> {
  return vi.spyOn(globalThis, "fetch").mockResolvedValue(okJson(SURAH1));
}

async function startReady(): Promise<FakeWorker> {
  const started = quranWorker.start(MANIFEST, QURAN_DATA.coordinates);
  const fake = FakeWorker.last!;
  const init = fake.posted.find(
    (m): m is Extract<WorkerRequest, { type: "init" }> => m.type === "init",
  )!;
  fake.emit("message", { id: init.id, ok: true, result: null });
  await started;
  return fake;
}

beforeEach(() => {
  vi.useFakeTimers();
  FakeWorker.last = null;
  vi.stubGlobal("Worker", FakeWorker);
});

afterEach(() => {
  const fake = FakeWorker.last;
  if (fake) {
    for (const msg of fake.posted) {
      if ("id" in msg) fake.emit("message", { id: msg.id, ok: true, result: null });
    }
  }
  quranWorker.dispose();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("quranWorker.readSurah API fallback", () => {
  it("serves from the live API when the wasm worker is not started (no-download mode)", async () => {
    const fetchMock = mockFetchSurah();
    const surah = await quranWorker.readSurah(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.test/quran/sources/uthmani/surah/1",
      expect.objectContaining({ headers: { accept: "application/json" } }),
    );
    expect(surah.sourceId).toBe("uthmani");
    expect(surah.verses).toEqual(["ٱلْحَمْدُ", "لِلَّهِ"]);
  });
});

describe("arabic fallback chain (worker + boot budget)", () => {
  it("never calls the API when the worker is healthy", async () => {
    const fake = await startReady();
    const fetchMock = mockFetchSurah();
    const onStatus = vi.fn();
    const p = quranWorker.readSurah(1, "uthmani", onStatus);
    await vi.advanceTimersByTimeAsync(0);
    const readReq = fake.posted.at(-1)!;
    expect(readReq.type).toBe("readSurah");
    fake.emit("message", { id: readReq.id, ok: true, result: SURAH1.data });
    const surah = await p;
    expect(surah.sourceId).toBe("uthmani");
    expect(fetchMock).not.toHaveBeenCalled();
    expect(onStatus).toHaveBeenCalledWith(expect.objectContaining({ servedBy: "local" }));
  });

  it("serves from the API when the worker reports a fatal error", async () => {
    const fake = await startReady();
    const fetchMock = mockFetchSurah();
    fake.emit("message", { type: "fatal", error: "engine exploded" });
    const surah = await quranWorker.readSurah(1, "uthmani");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(surah.sourceId).toBe("uthmani");
  });

  it("does not call the API when the worker becomes ready inside the boot budget", async () => {
    const fetchMock = mockFetchSurah();
    const started = quranWorker.start(MANIFEST, QURAN_DATA.coordinates);
    const fake = FakeWorker.last!;
    const p = quranWorker.readSurah(1, "uthmani");
    await vi.advanceTimersByTimeAsync(50);
    expect(fetchMock).not.toHaveBeenCalled();
    const init = fake.posted.find(
      (m): m is Extract<WorkerRequest, { type: "init" }> => m.type === "init",
    )!;
    fake.emit("message", { id: init.id, ok: true, result: null });
    await started;
    await vi.advanceTimersByTimeAsync(0);
    const readReq = fake.posted.at(-1)!;
    expect(readReq.type).toBe("readSurah");
    fake.emit("message", { id: readReq.id, ok: true, result: SURAH1.data });
    const surah = await p;
    expect(surah.sourceId).toBe("uthmani");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("falls through to the API when the boot budget expires before readiness", async () => {
    const fetchMock = mockFetchSurah();
    void quranWorker.start(MANIFEST, QURAN_DATA.coordinates);
    const p = quranWorker.readSurah(1, "uthmani");
    await vi.advanceTimersByTimeAsync(LOCAL_BOOT_BUDGET_MS);
    const surah = await p;
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(surah.sourceId).toBe("uthmani");
  });
});
