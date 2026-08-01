import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { OpenerKind, OpenerPackaging, QuranScript, QuranSourceId } from "$lib/data/quran-types";
import type { ResolvedManifest } from "$lib/quran/manifest";
import { quranWorker } from "$lib/quran/worker-client";
import type { WorkerOutbound, WorkerRequest } from "$lib/quran/protocol";
import { SearchHitKind, SearchProvider } from "$lib/quran/search/types";

/**
 * Drive quranWorker against a fake Worker boundary: we replace globalThis.Worker
 * with a recorder that captures posted messages and lets the test emit
 * message/error events, so settlement paths (response, timeout, fatal, disposal)
 * can be exercised without sqlite-wasm or a real thread.
 */

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

const RealWorker = globalThis.Worker;

const MANIFEST: ResolvedManifest = {
  contentVersion: "v1",
  searchVersion: "s1",
  scripts: [
    {
      id: QuranSourceId.TanzilUthmani,
      sizeBytes: 1,
      sha256: "a",
      downloadUrl: "https://x/uthmani",
    },
    {
      id: QuranSourceId.TanzilSimpleClean,
      sizeBytes: 1,
      sha256: "b",
      downloadUrl: "https://x/simple-clean",
    },
  ],
  source: "baked",
};

beforeEach(() => {
  vi.useFakeTimers();
  FakeWorker.last = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).Worker = FakeWorker;
});

afterEach(() => {
  quranWorker.dispose();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).Worker = RealWorker;
  vi.useRealTimers();
});

/** Start the worker and answer its init handshake so `ready` is true. */
async function startReady(): Promise<FakeWorker> {
  const started = quranWorker.start(MANIFEST);
  const fake = FakeWorker.last!;
  const init = fake.posted.find(
    (m): m is Extract<WorkerRequest, { type: "init" }> => m.type === "init",
  )!;
  fake.emit("message", { id: init.id, ok: true, result: null });
  await started;
  return fake;
}

describe("quranWorker request settlement", () => {
  it("resolves readSurah with the worker's verses", async () => {
    const fake = await startReady();
    expect(quranWorker.ready).toBe(true);

    const p = quranWorker.readSurah(1);
    const req = fake.posted.at(-1)!;
    const result = {
      sourceId: QuranSourceId.TanzilUthmani,
      script: QuranScript.Uthmani,
      verses: ["بسم", "الله"],
      normalization: {
        surah: 1,
        sourceId: QuranSourceId.TanzilUthmani,
        script: QuranScript.Uthmani,
        sourceProfile: "tanzil-uthmani-581cc540",
        packaging: OpenerPackaging.NumberedAyah,
        openerKind: OpenerKind.Verse,
        openerText: "بسم",
        openerEndScalar: 0,
        bodyStartScalar: 0,
      },
    } as const;
    fake.emit("message", { id: req.id, ok: true, result });

    expect(await p).toEqual(result);
  });

  it("rejects readSurah on an error response", async () => {
    const fake = await startReady();
    const p = quranWorker.readSurah(1);
    const req = fake.posted.at(-1)!;
    // Attach the handler before the synchronous emit rejects the promise.
    const assertion = expect(p).rejects.toThrow("no such surah");
    fake.emit("message", { id: req.id, ok: false, error: "no such surah" });
    await assertion;
  });

  it("decodes the canonical tagged search response", async () => {
    const fake = await startReady();
    const resultPromise = quranWorker.search("الم");
    const req = fake.posted.at(-1)!;
    fake.emit("message", {
      id: req.id,
      ok: true,
      result: {
        query: "الم",
        total: 1,
        limit: 20,
        offset: 0,
        results: [
          {
            kind: SearchHitKind.Ayah,
            ayah: {
              key: "2:1",
              surah: 2,
              ayah: 1,
              globalIndex: 8,
              text: "بِسْمِ ٱللَّهِ الٓمٓ",
            },
            highlights: [{ start: 16, end: 20 }],
          },
        ],
      },
    });

    expect(await resultPromise).toMatchObject({
      query: "الم",
      total: 1,
      source: SearchProvider.Worker,
      results: [{ kind: SearchHitKind.Ayah, ayah: { key: "2:1" } }],
    });
  });

  it("rejects a legacy ayah-only search response", async () => {
    const fake = await startReady();
    const resultPromise = quranWorker.search("الم");
    const assertion = expect(resultPromise).rejects.toThrow("malformed search response");
    const req = fake.posted.at(-1)!;
    fake.emit("message", {
      id: req.id,
      ok: true,
      result: {
        query: "الم",
        total: 1,
        limit: 20,
        offset: 0,
        results: [{ ayah: { key: "2:1" }, highlights: [] }],
      },
    });
    await assertion;
  });

  it("rejects a request that never gets a response (timeout)", async () => {
    await startReady();
    const p = quranWorker.readSurah(1);
    // Attach the handler before advancing the fake clock: the timer fires
    // inside advanceTimersByTimeAsync, so attaching later would briefly leave
    // the rejection unhandled.
    const assertion = expect(p).rejects.toThrow("timed out");
    await vi.advanceTimersByTimeAsync(30_000);
    await assertion;
  });

  it("a post-init fatal rejects every in-flight request", async () => {
    const fake = await startReady();
    const p1 = quranWorker.readSurah(1);
    const p2 = quranWorker.readSurah(2);
    // Attach handlers before the fatal event rejects both synchronously.
    const a1 = expect(p1).rejects.toThrow("sqlite-wasm exploded");
    const a2 = expect(p2).rejects.toThrow("sqlite-wasm exploded");
    fake.emit("message", { type: "fatal", error: "sqlite-wasm exploded" });
    await a1;
    await a2;
  });

  it("dispose rejects in-flight requests and terminates the worker", async () => {
    const fake = await startReady();
    const p = quranWorker.readSurah(1);
    const assertion = expect(p).rejects.toThrow("disposed");
    quranWorker.dispose();
    await assertion;
    expect(fake.terminated).toBe(true);
    expect(quranWorker.ready).toBe(false);
  });
});
