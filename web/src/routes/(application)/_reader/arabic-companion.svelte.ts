import type { QuranScript, VerseKey } from "$lib/data/quran-types";
import { LOCAL_HEDGE_BUDGET_MS } from "$lib/quran/fetch";
import type { WorkerStatus } from "$lib/quran/protocol";
import { bodyText } from "$lib/quran/view/source-view";
import type { AyahCoordinateValidator } from "$lib/quran/wire";
import { quranWorker } from "$lib/quran/worker-client";
import { untrack } from "svelte";

/**
 * The Arabic for a translation route. A /t/ page's own text is its translation (SSR, SEO),
 * but Ayah-by-Ayah never drops the Arabic: this fetches the reader's Arabic script for the
 * loaded range so each row leads with the Arabic and the route's translation becomes the
 * first lane. Opener (bismillah) prefixes are stripped exactly like the Arabic route.
 */
export interface ArabicAyah {
  readonly text: string;
  readonly script: QuranScript;
}

export interface ArabicCompanionState {
  readonly byKey: ReadonlyMap<VerseKey, ArabicAyah>;
}

export interface ArabicCompanion {
  readonly state: ArabicCompanionState;
  sync(): void;
  dispose(): void;
}

export interface CreateArabicCompanionOptions {
  /** Fetch only while it is needed (a translation route in Ayah-by-Ayah). */
  readonly enabled: () => boolean;
  /** The reader's Arabic script source id. */
  readonly source: () => string;
  readonly from: () => number;
  readonly to: () => number;
  readonly validator: () => AyahCoordinateValidator | null;
  readonly routeKey: () => string;
}

export function createArabicCompanion(opts: CreateArabicCompanionOptions): ArabicCompanion {
  let byKey = $state.raw<Map<VerseKey, ArabicAyah>>(new Map());
  let gen = 0;
  let disposed = false;
  let lastIdentity = "";
  let lastFrom = -1;
  let lastTo = -1;
  let failed = false;
  let inFlight: AbortController | null = null;
  let stopWorkerStatus: (() => void) | null = null;

  function fetchRange(
    from: number,
    to: number,
    source: string,
    validator: AyahCoordinateValidator,
  ): void {
    const startGen = ++gen;
    inFlight?.abort();
    const controller = new AbortController();
    inFlight = controller;
    quranWorker
      .readRange(from, to, validator, source, undefined, {
        hedgeAfterMs: LOCAL_HEDGE_BUDGET_MS,
        signal: controller.signal,
      })
      .then((range) => {
        if (disposed || startGen !== gen) return;
        inFlight = null;
        failed = false;
        const normalizations = new Map(range.normalizations.map((n) => [n.surah, n]));
        const next = new Map(untrack(() => byKey));
        for (const ayah of range.ayahs) {
          const normalization = normalizations.get(ayah.surah);
          if (!normalization) continue;
          next.set(ayah.key, {
            text: bodyText(ayah.text, ayah.ayah, normalization),
            script: normalization.script,
          });
        }
        byKey = next;
      })
      .catch(() => {
        if (disposed || startGen !== gen) return;
        inFlight = null;
        failed = true;
      });
  }

  function onWorkerStatus(status: WorkerStatus): void {
    // A cold first read can fail while the Arabic artifact is still landing; retry once warm.
    if (disposed || status !== "ready" || !failed) return;
    lastFrom = -1;
    lastTo = -1;
    sync();
  }

  function sync(): void {
    if (!stopWorkerStatus && !disposed)
      stopWorkerStatus = quranWorker.onStatus?.(onWorkerStatus) ?? null;
    const identity = `${opts.routeKey()}|${opts.source()}`;
    if (identity !== lastIdentity) {
      lastIdentity = identity;
      gen++;
      inFlight?.abort();
      inFlight = null;
      byKey = new Map();
      lastFrom = -1;
      lastTo = -1;
    }
    if (!opts.enabled()) return;
    const validator = opts.validator();
    const from = opts.from();
    const to = opts.to();
    if (!validator || from < 1 || to < from) return;
    if (from === lastFrom && to === lastTo) return;
    lastFrom = from;
    lastTo = to;
    fetchRange(from, to, opts.source(), validator);
  }

  return {
    state: {
      get byKey() {
        return byKey;
      },
    },
    sync,
    dispose(): void {
      disposed = true;
      gen++;
      inFlight?.abort();
      inFlight = null;
      stopWorkerStatus?.();
      stopWorkerStatus = null;
    },
  };
}
