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

const TRANSLATION = "en.pickthall";

async function importEngagement() {
  return import("$lib/quran/engagement");
}

/** whenIdle uses requestIdleCallback when present; force the setTimeout path. */
function stubIdle(): void {
  Reflect.deleteProperty(window, "requestIdleCallback");
  vi.stubGlobal("setTimeout", ((fn: () => void) => {
    fn();
    return 0;
  }) as unknown as typeof setTimeout);
}

async function flush(): Promise<void> {
  for (let i = 0; i < 6; i++) await Promise.resolve();
}

describe("reader engagement prefetch", () => {
  beforeEach(async () => {
    vi.resetModules();
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

  it("does not prefetch for a single view (bounce)", async () => {
    const { noteReaderView } = await importEngagement();
    await noteReaderView(TRANSLATION);
    await flush();
    expect(ensureTranslation).not.toHaveBeenCalled();
  });

  it("prefetches once the visitor reaches a second view", async () => {
    const { noteReaderView } = await importEngagement();
    await noteReaderView(TRANSLATION);
    await noteReaderView(TRANSLATION);
    await flush();
    expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
  });

  it("counts Arabic views toward engagement without prefetching Arabic", async () => {
    const { noteReaderView } = await importEngagement();
    await noteReaderView(QuranSourceId.TanzilUthmani);
    await flush();
    expect(ensureTranslation).not.toHaveBeenCalled();

    await noteReaderView(TRANSLATION);
    await flush();
    expect(ensureTranslation).toHaveBeenCalledWith(TRANSLATION);
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
    await noteReaderView(TRANSLATION);
    await flush();
    expect(ensureTranslation).toHaveBeenCalledTimes(1);
    expect(sessionStorage.getItem(`eq:tprefetch:${TRANSLATION}`)).toBeNull();

    await noteReaderView(TRANSLATION);
    await flush();
    expect(ensureTranslation).toHaveBeenCalledTimes(2);
  });

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
