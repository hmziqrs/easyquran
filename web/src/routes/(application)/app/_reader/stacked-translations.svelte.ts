import { untrack } from "svelte";
import type {
  StackedSourceState,
  StackedTranslation,
  StackedTranslationsByVerse,
  TranslationCatalogueEntry,
  VerseKey,
} from "$lib/data/quran-types";
import { LOCAL_HEDGE_BUDGET_MS } from "$lib/quran/fetch";
import type { WorkerStatus } from "$lib/quran/protocol";
import { quranWorker } from "$lib/quran/worker-client";
import type { AyahCoordinateValidator } from "$lib/quran/wire";
import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";

export interface StackedControllerState {
  readonly byVerse: StackedTranslationsByVerse;
  readonly order: readonly string[];
  readonly status: ReadonlyMap<string, StackedSourceState>;
}

export interface StackedController {
  readonly state: StackedControllerState;
  sync(): void;
  dispose(): void;
}

export interface CreateStackedTranslationsOptions {
  readonly from: () => number;
  readonly to: () => number;
  readonly validator: () => AyahCoordinateValidator | null;
  readonly primarySourceId: () => string | null;
  readonly catalogue: () => readonly TranslationCatalogueEntry[];
  readonly routeKey: () => string;
}

export function createStackedTranslations(
  opts: CreateStackedTranslationsOptions,
): StackedController {
  let byVerse = $state<Map<VerseKey, StackedTranslation[]>>(new Map());
  let order = $state<string[]>([]);
  let status = $state<Map<string, StackedSourceState>>(new Map());

  let gen = 0;
  let disposed = false;
  let prevRoute = "";
  let lastFrom = -1;
  let lastTo = -1;
  const metaCache = new Map<string, TranslationCatalogueEntry>();
  const ERROR_RETRY_DELAYS_MS: readonly number[] = [1200, 4000, 9000];
  const WARM_RETRY_CAP = 3;
  const retryAttempts = new Map<string, number>();
  const retryTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const warmRetries = new Map<string, number>();
  const requestSeq = new Map<string, number>();
  const inFlight = new Map<string, AbortController>();
  let stopWorkerStatus: (() => void) | null = null;

  function metaFor(id: string): TranslationCatalogueEntry | undefined {
    const cached = metaCache.get(id);
    if (cached) return cached;
    const found = opts.catalogue().find((entry) => entry.id === id);
    if (found) metaCache.set(id, found);
    return found;
  }

  function setStatus(id: string, value: StackedSourceState): void {
    const next = new Map(status);
    next.set(id, value);
    status = next;
  }

  function mergeRange(
    id: string,
    ayahs: readonly { readonly key: VerseKey; readonly text: string }[],
  ): void {
    const meta = metaFor(id);
    if (!meta) return;
    const next = new Map(byVerse);
    for (const ayah of ayahs) {
      const existing = next.get(ayah.key) ?? [];
      const filtered = existing.filter((t) => t.sourceId !== id);
      filtered.push({
        sourceId: id,
        translator: meta.translator,
        language: meta.language,
        languageCode: meta.languageCode,
        direction: meta.direction,
        text: ayah.text,
      });
      next.set(ayah.key, filtered);
    }
    byVerse = next;
  }

  function fetchExtra(
    id: string,
    fromVal: number,
    toVal: number,
    validator: AyahCoordinateValidator,
  ): void {
    const startGen = gen;
    const token = (requestSeq.get(id) ?? 0) + 1;
    requestSeq.set(id, token);
    inFlight.get(id)?.abort();
    const controller = new AbortController();
    inFlight.set(id, controller);
    const stale = (): boolean => disposed || startGen !== gen || requestSeq.get(id) !== token;
    const settle = (): void => {
      if (inFlight.get(id) === controller) inFlight.delete(id);
    };
    quranWorker
      .readRange(fromVal, toVal, validator, id, undefined, {
        hedgeAfterMs: LOCAL_HEDGE_BUDGET_MS,
        signal: controller.signal,
      })
      .then((range) => {
        settle();
        if (stale()) return;
        if (!metaFor(id)) {
          setStatus(id, "error");
          scheduleErrorRetry(id, fromVal, toVal, validator);
          return;
        }
        mergeRange(id, range.ayahs);
        setStatus(id, "ready");
        clearErrorRetry(id);
        warmRetries.delete(id);
      })
      .catch(() => {
        settle();
        if (stale()) return;
        setStatus(id, "error");
        scheduleErrorRetry(id, fromVal, toVal, validator);
      });
  }

  function scheduleErrorRetry(
    id: string,
    fromVal: number,
    toVal: number,
    validator: AyahCoordinateValidator,
  ): void {
    const attempt = retryAttempts.get(id) ?? 0;
    const delay = ERROR_RETRY_DELAYS_MS[attempt];
    if (delay === undefined) return;
    retryAttempts.set(id, attempt + 1);
    const startGen = gen;
    const timer = setTimeout(() => {
      retryTimers.delete(id);
      if (disposed || startGen !== gen) return;
      if (untrack(() => status.get(id)) !== "error") return;
      fetchExtra(id, fromVal, toVal, validator);
    }, delay);
    retryTimers.set(id, timer);
  }

  function clearErrorRetry(id: string): void {
    const timer = retryTimers.get(id);
    if (timer !== undefined) clearTimeout(timer);
    retryTimers.delete(id);
    retryAttempts.delete(id);
  }

  /**
   * The worker goes back to "ready" when the last artifact download settles, so a source that
   * exhausted its timed retries while its DB was still landing gets one more chance per warm-up.
   * Status is deliberately left at "error" (never re-flipped to "loading"): VerseRow renders
   * skeleton and text rows additively, so a re-flip would double-render a source that has text.
   */
  function retryColdExtras(): void {
    const validator = opts.validator();
    if (!validator) return;
    if (lastFrom < 1 || lastTo < lastFrom) return;
    for (const id of untrack(() => order)) {
      if (untrack(() => status.get(id)) !== "error") continue;
      if (inFlight.has(id)) continue;
      const attempts = warmRetries.get(id) ?? 0;
      if (attempts >= WARM_RETRY_CAP) continue;
      warmRetries.set(id, attempts + 1);
      clearErrorRetry(id);
      fetchExtra(id, lastFrom, lastTo, validator);
    }
  }

  function handleWorkerStatus(workerStatus: WorkerStatus, detail?: string): void {
    if (disposed) return;
    if (workerStatus === "translation-fetch-failed") {
      // A failed download is followed by "ready"; hand that source a fresh timed budget.
      if (detail !== undefined) retryAttempts.delete(detail);
      return;
    }
    if (workerStatus !== "ready") return;
    retryColdExtras();
  }

  function abortInFlight(id: string): void {
    const controller = inFlight.get(id);
    if (!controller) return;
    controller.abort();
    inFlight.delete(id);
  }

  function ensureWorkerStatusSubscription(): void {
    if (stopWorkerStatus || disposed) return;
    stopWorkerStatus = quranWorker.onStatus?.(handleWorkerStatus) ?? null;
  }

  function sync(): void {
    ensureWorkerStatusSubscription();
    const route = opts.routeKey();
    const fromVal = opts.from();
    const toVal = opts.to();
    const primary = opts.primarySourceId();
    const extras = stackedTranslations.ids.filter((id) => id !== primary);

    const prevOrder = untrack(() => order);
    const sameOrder =
      prevOrder.length === extras.length && prevOrder.every((id, i) => id === extras[i]);
    if (!sameOrder) {
      order = extras;
      const pruned = untrack(() => {
        const next = new Map(status);
        const dropped: string[] = [];
        for (const id of next.keys()) {
          if (!extras.includes(id)) {
            next.delete(id);
            dropped.push(id);
          }
        }
        return { next, dropped };
      });
      status = pruned.next;
      for (const id of pruned.dropped) {
        clearErrorRetry(id);
        abortInFlight(id);
        warmRetries.delete(id);
      }
    }

    if (route !== prevRoute) {
      prevRoute = route;
      gen++;
      byVerse = new Map();
      status = new Map();
      for (const id of retryTimers.keys()) clearErrorRetry(id);
      warmRetries.clear();
    }

    const validator = opts.validator();
    if (!validator) return;

    const rangeChanged = fromVal !== lastFrom || toVal !== lastTo;
    lastFrom = fromVal;
    lastTo = toVal;

    for (const id of extras) {
      const cur = untrack(() => status.get(id));
      if (cur === undefined) {
        setStatus(id, "loading");
        fetchExtra(id, fromVal, toVal, validator);
      } else if ((cur === "ready" || cur === "loading") && rangeChanged) {
        fetchExtra(id, fromVal, toVal, validator);
      }
    }
  }

  return {
    state: {
      get byVerse() {
        return byVerse;
      },
      get order() {
        return order;
      },
      get status() {
        return status;
      },
    },
    sync,
    dispose(): void {
      disposed = true;
      gen++;
      for (const id of retryTimers.keys()) clearErrorRetry(id);
      for (const controller of inFlight.values()) controller.abort();
      inFlight.clear();
      requestSeq.clear();
      warmRetries.clear();
      stopWorkerStatus?.();
      stopWorkerStatus = null;
    },
  };
}

export function stackedFor(
  state: StackedControllerState,
  key: VerseKey,
): readonly StackedTranslation[] {
  const list = state.byVerse.get(key);
  if (!list || list.length === 0) return [];
  const ord = state.order;
  return list
    .filter((t) => ord.includes(t.sourceId))
    .sort((a, b) => ord.indexOf(a.sourceId) - ord.indexOf(b.sourceId));
}

export function loadingFor(state: StackedControllerState, _key: VerseKey): readonly string[] {
  return state.order.filter((id) => state.status.get(id) === "loading");
}

export function erroredFor(state: StackedControllerState, key: VerseKey): readonly string[] {
  const withText = new Set((state.byVerse.get(key) ?? []).map((t) => t.sourceId));
  return state.order.filter((id) => state.status.get(id) === "error" && !withText.has(id));
}
