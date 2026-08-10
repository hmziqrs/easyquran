import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { QuranSourceId } from "$lib/data/quran-types";

const hasTranslation = vi.fn<(source: string) => Promise<boolean>>();
const ensureTranslation = vi.fn<(source: string) => Promise<void>>();
const onStatusDetachers: Array<() => void> = [];
type StatusCallback = (status: string, detail?: string) => void;
let statusCb: StatusCallback | undefined;

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$lib/quran/worker-client", () => ({
  quranWorker: {
    whenReady: () => Promise.resolve(),
    hasTranslation: (source: string) => hasTranslation(source),
    ensureTranslation: (source: string) => ensureTranslation(source),
    onStatus: (cb: StatusCallback): (() => void) => {
      statusCb = cb;
      const detach = (): void => {
        if (statusCb === cb) statusCb = undefined;
      };
      onStatusDetachers.push(detach);
      return detach;
    },
  },
}));

// writeJSON is wrapped so the failed-durable-write test can simulate a silent
// no-op write (the real failure mode: safe-storage catches and returns). All
// other tests use the real implementation.
vi.mock("$lib/storage", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/storage")>();
  return { ...actual, writeJSON: vi.fn(actual.writeJSON) };
});

const TRANSLATION = "en.pickthall";
const ENGAGEMENT_KEY = "eq:engagement";
const SESSION_KEY = "eq:reader-session-views";
const LEGACY_KEY = "eq:reader-views";
const READER_KEY = "easyquran.reader";
const SOURCE_KEY = "easyquran.reader.source";

async function importEngagement() {
  return import("$lib/quran/engagement");
}

function stubIdle(): void {
  Reflect.deleteProperty(window, "requestIdleCallback");
  vi.stubGlobal("setTimeout", ((fn: () => void) => {
    fn();
    return 0;
  }) as unknown as typeof setTimeout);
}

async function flush(): Promise<void> {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}

function ymd(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate(),
  ).padStart(2, "0")}`;
}
function todayStr(): string {
  return ymd(new Date());
}
function yesterdayStr(): string {
  return ymd(new Date(Date.now() - 86_400_000));
}

interface DurableSeed {
  totalViews?: number;
  distinctDays?: number;
  lastDay?: string;
  qualified?: boolean;
  legacySeeded?: boolean;
  sourceViews?: Record<string, number>;
}

function seedDurable(p: DurableSeed = {}): void {
  const blob = {
    v: 1,
    totalViews: p.totalViews ?? 0,
    distinctDays: p.distinctDays ?? 0,
    lastDay: p.lastDay ?? "",
    firstSeen: Date.now(),
    lastSeen: Date.now(),
    qualified: p.qualified ?? false,
    legacySeeded: p.legacySeeded ?? true,
    sourceViews: p.sourceViews ?? {},
  };
  window.localStorage.setItem(ENGAGEMENT_KEY, JSON.stringify(blob));
}

interface DurableBlob {
  totalViews: number;
  distinctDays: number;
  sourceViews: Record<string, number>;
  [k: string]: unknown;
}

function readDurable(): DurableBlob | undefined {
  const raw = window.localStorage.getItem(ENGAGEMENT_KEY);
  return raw ? (JSON.parse(raw) as DurableBlob) : undefined;
}

function seedEngagedReader(translation: string): void {
  seedDurable({
    totalViews: 5,
    distinctDays: 1,
    lastDay: todayStr(),
    qualified: false,
    sourceViews: { [translation]: 2 },
  });
}

function seedReaderState(opts: {
  bookmarks?: Record<string, boolean>;
  notes?: Record<string, string>;
  lastRead?: unknown;
  sourceId?: string | null;
}): void {
  window.localStorage.setItem(
    READER_KEY,
    JSON.stringify({
      v: 1,
      current: 1,
      fontSize: 33,
      mode: "verse",
      bookmarks: opts.bookmarks ?? {},
      notes: opts.notes ?? {},
      lastRead: opts.lastRead === undefined ? null : opts.lastRead,
    }),
  );
  if (opts.sourceId !== undefined) {
    window.localStorage.setItem(SOURCE_KEY, JSON.stringify({ v: 1, sourceId: opts.sourceId }));
  }
}

describe("reader engagement", () => {
  beforeEach(async () => {
    vi.resetModules();
    window.localStorage.clear();
    sessionStorage.clear();
    hasTranslation.mockReset();
    ensureTranslation.mockReset();
    hasTranslation.mockResolvedValue(false);
    ensureTranslation.mockResolvedValue(undefined);
    for (const detach of onStatusDetachers.splice(0)) detach();
    statusCb = undefined;
    stubIdle();
    const mod = await importEngagement();
    mod.__resetEngagementState();
    await flush();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe("gate", () => {
    it("does not prefetch for a single view on a single day", async () => {
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
    });

    it("prefetches once the visitor reaches four views on the same day", async () => {
      const { noteReaderView } = await importEngagement();
      for (let i = 0; i < 3; i++) {
        await noteReaderView(TRANSLATION);
        await flush();
      }
      expect(ensureTranslation).not.toHaveBeenCalled();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
    });

    it("prefetches for a second view on a second distinct day", async () => {
      seedDurable({
        totalViews: 1,
        distinctDays: 1,
        lastDay: yesterdayStr(),
        sourceViews: { [TRANSLATION]: 1 },
      });
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
      const d = readDurable();
      expect(d?.distinctDays).toBe(2);
      expect(d?.lastDay).toBe(todayStr());
    });

    it("never prefetches Arabic-only views even when the reader is engaged", async () => {
      seedEngagedReader(TRANSLATION);
      const { noteReaderView } = await importEngagement();
      for (let i = 0; i < 6; i++) {
        await noteReaderView(QuranSourceId.TanzilUthmani);
        await flush();
      }
      expect(ensureTranslation).not.toHaveBeenCalled();
      const d = readDurable();
      expect(d?.totalViews).toBe(11);
      expect(d?.sourceViews[TRANSLATION]).toBe(2);
      expect(d?.sourceViews[QuranSourceId.TanzilUthmani]).toBeUndefined();
    });

    it("reads the pre-bump sourceViews count for the gate", async () => {
      seedDurable({ totalViews: 10, distinctDays: 1, sourceViews: {} });
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
      expect(readDurable()?.sourceViews[TRANSLATION]).toBe(1);

      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
    });
  });

  describe("migration and seeding", () => {
    it("seeds totalViews from the legacy key exactly once", async () => {
      sessionStorage.setItem(LEGACY_KEY, "2");
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      const d = readDurable();
      expect(d?.totalViews).toBe(3);
      expect(d?.legacySeeded).toBe(true);
      expect(sessionStorage.getItem(LEGACY_KEY)).toBeNull();

      await noteReaderView(TRANSLATION);
      await flush();
      expect(readDurable()?.totalViews).toBe(4);
    });

    it("falls back to session-only without throwing when the durable read yields undefined", async () => {
      const desc = Object.getOwnPropertyDescriptor(Storage.prototype, "getItem");
      Object.defineProperty(Storage.prototype, "getItem", {
        configurable: true,
        value: () => {
          throw new DOMException("insecure", "SecurityError");
        },
      });
      let threw = false;
      try {
        const { noteReaderView } = await importEngagement();
        await noteReaderView(TRANSLATION);
      } catch {
        threw = true;
      } finally {
        if (desc) Object.defineProperty(Storage.prototype, "getItem", desc);
      }
      expect(threw).toBe(false);
      expect(sessionStorage.getItem(SESSION_KEY)).not.toBeNull();
    });

    it("retains the legacy key when the durable write fails", async () => {
      sessionStorage.setItem(LEGACY_KEY, "2");
      const { writeJSON } = await import("$lib/storage");
      const mocked = vi.mocked(writeJSON);
      // Simulate the real failure mode: safe-storage swallows the write error
      // and returns, so nothing is persisted — the read-back then fails and the
      // legacy key must survive for the next retry.
      mocked.mockImplementation(async () => {});
      try {
        const { noteReaderView } = await importEngagement();
        await noteReaderView(TRANSLATION);
        expect(sessionStorage.getItem(LEGACY_KEY)).toBe("2");
      } finally {
        mocked.mockRestore();
      }
    });

    it("repeat-load migration seeds durable history once while the new session counter continues", async () => {
      // A repeat "load" = a fresh module instance against the same browser
      // storage. The legacy counter must seed the durable total exactly once
      // (the first confirmed load); every subsequent load must only add its own
      // view, while the session counter keeps accumulating across all loads.
      sessionStorage.setItem(LEGACY_KEY, "2");

      const loads = 4;
      let lastSession = -1;
      for (let i = 0; i < loads; i++) {
        vi.resetModules();
        const { noteReaderView } = await importEngagement();
        await noteReaderView(TRANSLATION);
        await flush();

        const d = readDurable();
        expect(d?.legacySeeded).toBe(true);
        // durable.totalViews = legacy seed (2) applied once + one view per load.
        expect(d?.totalViews).toBe(2 + (i + 1));
        // The session counter never resets and increments by one each load.
        const s = Number(sessionStorage.getItem(SESSION_KEY) ?? 0);
        expect(s).toBe(2 + (i + 1));
        expect(s).toBeGreaterThan(lastSession);
        lastSession = s;
      }

      // Legacy key removed after the first confirmed durable read-back and
      // never reappears on later loads.
      expect(sessionStorage.getItem(LEGACY_KEY)).toBeNull();
    });

    it("round-trips numeric source counts and drops garbage on decode", async () => {
      seedDurable({ totalViews: 5, sourceViews: { a: 3, b: 5 } });
      const { noteReaderView } = await importEngagement();
      await noteReaderView("a");
      await flush();
      const d = readDurable();
      expect(d?.sourceViews).toMatchObject({ a: 4, b: 5 });

      window.localStorage.setItem(
        ENGAGEMENT_KEY,
        JSON.stringify({
          v: 1,
          totalViews: 1,
          distinctDays: 1,
          lastDay: todayStr(),
          firstSeen: 1,
          lastSeen: 1,
          qualified: false,
          legacySeeded: true,
          sourceViews: { a: "x", b: 2.5, c: 5, d: -1 },
        }),
      );
      await noteReaderView("c");
      await flush();
      expect(readDurable()?.sourceViews).toEqual({ c: 6 });
    });

    it("timestamp-free lastRead never invents a visit day", async () => {
      seedReaderState({
        lastRead: { num: 2, n: 5, sourceId: TRANSLATION },
      });
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      const d = readDurable();
      expect(d?.distinctDays).toBe(1);
      expect(d?.lastDay).toBe(todayStr());
    });

    it("bookmarks/notes qualify the reader and the chosen translation seeds only its source", async () => {
      seedReaderState({
        bookmarks: { "1:1": true },
        sourceId: TRANSLATION,
      });
      const { noteReaderView } = await importEngagement();
      await noteReaderView("fr.hamidullah");
      await flush();
      const d = readDurable();
      expect(d?.qualified).toBe(true);
      expect(d?.sourceViews[TRANSLATION]).toBe(1);
      expect(d?.sourceViews["fr.hamidullah"]).toBe(1);

      seedReaderState({
        bookmarks: { "1:1": true },
        sourceId: QuranSourceId.TanzilUthmani,
      });
      window.localStorage.removeItem(ENGAGEMENT_KEY);
      sessionStorage.removeItem(LEGACY_KEY);
      await noteReaderView("fr.hamidullah");
      await flush();
      const d2 = readDurable();
      expect(d2?.sourceViews[QuranSourceId.TanzilUthmani]).toBeUndefined();
      expect(d2?.sourceViews["fr.hamidullah"]).toBe(1);
    });

    it("a notes-only reader (no bookmarks) qualifies the reader and seeds the chosen translation", async () => {
      seedReaderState({
        notes: { "2:255": "remember this verse" },
        sourceId: TRANSLATION,
      });
      const { noteReaderView } = await importEngagement();
      await noteReaderView("fr.hamidullah");
      await flush();
      const d = readDurable();
      expect(d?.qualified).toBe(true);
      expect(d?.sourceViews[TRANSLATION]).toBe(1);
      expect(d?.sourceViews["fr.hamidullah"]).toBe(1);
    });
  });

  describe("prefetch orchestration", () => {
    beforeEach(() => {
      seedEngagedReader(TRANSLATION);
    });

    it("fires once when the request is accepted, no matter how many views follow", async () => {
      const { noteReaderView } = await importEngagement();
      for (let i = 0; i < 6; i++) {
        await noteReaderView(TRANSLATION);
        await flush();
      }
      expect(ensureTranslation).toHaveBeenCalledTimes(1);
    });

    it("retries a worker-infra-failed ensureTranslation at most once per tab", async () => {
      ensureTranslation.mockRejectedValue(new Error("worker down"));
      const { noteReaderView } = await importEngagement();
      for (let i = 0; i < 6; i++) {
        await noteReaderView(TRANSLATION);
        await flush();
      }
      expect(ensureTranslation).toHaveBeenCalledTimes(2);
    });

    it("registers the onStatus listener on module load", async () => {
      await importEngagement();
      await flush();
      expect(statusCb).toBeTypeOf("function");
    });

    it("translation-fetch-failed status clears the settled flag so the next view retries", async () => {
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(1);
      expect(sessionStorage.getItem(`eq:tprefetch:${TRANSLATION}`)).toBe("1");

      statusCb?.("translation-fetch-failed", TRANSLATION);
      await flush();
      expect(sessionStorage.getItem(`eq:tprefetch:${TRANSLATION}`)).toBeNull();

      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(2);
    });

    it("caps retries at one extra fetch on the status-driven path", async () => {
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(1);

      statusCb?.("translation-fetch-failed", TRANSLATION);
      await flush();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(2);

      statusCb?.("translation-fetch-failed", TRANSLATION);
      await flush();
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(2);
    });

    it("ignores unrelated statuses and detail-less translation-fetch-failed", async () => {
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      const callsBefore = ensureTranslation.mock.calls.length;

      statusCb?.("ready");
      statusCb?.("translation-fetch-failed");
      statusCb?.("translation-fetch-failed", "");
      await flush();

      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation.mock.calls.length).toBe(callsBefore);
    });

    it("never downloads a translation that is already cached", async () => {
      hasTranslation.mockResolvedValue(true);
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
    });

    it("stops consulting the worker once a source is settled", async () => {
      hasTranslation.mockResolvedValue(true);
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      const callsAfterFirst = hasTranslation.mock.calls.length;

      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(hasTranslation.mock.calls.length).toBe(callsAfterFirst);
    });

    it("skips a fresh tab when the source is already flagged in sessionStorage", async () => {
      sessionStorage.setItem(`eq:tprefetch:${TRANSLATION}`, "1");
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
      expect(hasTranslation).not.toHaveBeenCalled();
    });

    it("respects Save-Data", async () => {
      vi.stubGlobal("navigator", { connection: { saveData: true } });
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
    });

    it("respects slow connections", async () => {
      vi.stubGlobal("navigator", { connection: { effectiveType: "2g" } });
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
    });

    it("clears the flag when the download fails so a later view can retry", async () => {
      ensureTranslation.mockRejectedValue(new Error("network"));
      const { noteReaderView } = await importEngagement();
      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(1);
      expect(sessionStorage.getItem(`eq:tprefetch:${TRANSLATION}`)).toBeNull();

      await noteReaderView(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledTimes(2);
    });
  });

  describe("explicit translation choice", () => {
    it("prefetches immediately when the reader picks a translation by hand", async () => {
      const { noteTranslationChosen } = await importEngagement();
      await noteTranslationChosen(TRANSLATION);
      await flush();
      expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
    });

    it("does not re-download a hand-picked translation that is cached", async () => {
      hasTranslation.mockResolvedValue(true);
      const { noteTranslationChosen } = await importEngagement();
      await noteTranslationChosen(TRANSLATION);
      await flush();
      expect(ensureTranslation).not.toHaveBeenCalled();
    });
  });
});

describe("downloadBytes", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("completes an in-budget streaming download", async () => {
    vi.spyOn(globalThis, "fetch").mockImplementation(() => {
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(new Uint8Array(60));
          controller.enqueue(new Uint8Array(40));
          controller.close();
        },
      });
      return Promise.resolve(new Response(body, { status: 200 }));
    });
    const { downloadBytes } = await import("$lib/workers/download");
    const buf = await downloadBytes({ url: "https://x.test/d", sizeBytes: 100, label: "d" });
    expect(buf.byteLength).toBe(100);
  });

  it("aborts when the elapsed budget expires", async () => {
    let observed: AbortSignal | null | undefined;
    vi.spyOn(globalThis, "fetch").mockImplementation((_url, init) => {
      observed = (init as RequestInit | undefined)?.signal;
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          const sig = observed;
          if (sig?.aborted) controller.error(new DOMException("aborted", "AbortError"));
          sig?.addEventListener("abort", () =>
            controller.error(new DOMException("aborted", "AbortError")),
          );
        },
        pull() {
          return new Promise<void>(() => {});
        },
      });
      return Promise.resolve(new Response(body, { status: 200 }));
    });
    const { downloadBytes, DOWNLOAD_BUDGET_MS } = await import("$lib/workers/download");
    const p = downloadBytes({ url: "https://x.test/d", sizeBytes: 100, label: "d" });
    p.catch(() => {});
    await vi.advanceTimersByTimeAsync(DOWNLOAD_BUDGET_MS);
    await expect(p).rejects.toThrow(/aborted/i);
  });

  it("aborts an oversized stream before unbounded allocation", async () => {
    let enqueued = 0;
    vi.spyOn(globalThis, "fetch").mockImplementation(() => {
      const body = new ReadableStream<Uint8Array>({
        pull(controller) {
          controller.enqueue(new Uint8Array(80));
          enqueued++;
          if (enqueued >= 5) controller.close();
        },
      });
      return Promise.resolve(new Response(body, { status: 200 }));
    });
    const { downloadBytes } = await import("$lib/workers/download");
    await expect(
      downloadBytes({ url: "https://x.test/d", sizeBytes: 100, label: "d" }),
    ).rejects.toThrow(/exceeded declared 100 bytes/);
    expect(enqueued).toBeLessThan(5);
  });

  it("enforces the byte ceiling on the arrayBuffer path before allocation", async () => {
    // When res.body is absent, downloadBytes falls back to res.arrayBuffer(),
    // which materializes the full body in one allocation. A Content-Length
    // announcing an oversized body must be rejected BEFORE that allocation on
    // this branch — the streaming path's pre-allocation guard does not run here.
    vi.spyOn(globalThis, "fetch").mockResolvedValue({
      ok: true,
      status: 200,
      body: null,
      headers: new Headers({ "content-length": "200" }),
      arrayBuffer: async () => new ArrayBuffer(200),
    } as unknown as Response);
    const { downloadBytes } = await import("$lib/workers/download");
    await expect(
      downloadBytes({ url: "https://x.test/d", sizeBytes: 100, label: "d" }),
    ).rejects.toThrow(/Content-Length 200 exceeds declared 100 bytes/);
  });
});
