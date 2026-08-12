import type { SurahRouteContext } from "$lib/data/quran";
import type { QuranData } from "$lib/data/quran-data";
import { loadQuranData } from "$lib/data/quran-data-client";
import { parseQuery } from "./query";
import {
  DEFAULT_SOURCE_LIMIT,
  asyncSources,
  collectAsyncEntries,
  collectSyncEntries,
  dedupeEntries,
  sectionsFor,
  type PaletteSection,
} from "./registry";
import type { PaletteEntry, PaletteQuery } from "./types";

export const SEARCH_DEBOUNCE_MS = 140;

export interface PaletteEngineInput {
  /** Raw query text as typed. */
  query: () => string;
  /** Only the open palette loads data and runs sources. */
  open: () => boolean;
  /** Active reader route context, so jumps keep the current translation. */
  routeContext: () => SurahRouteContext;
}

export interface PaletteEngine {
  readonly ready: boolean;
  /** True when the Quran catalogue could not be fetched for this open. */
  readonly catalogueFailed: boolean;
  readonly searching: boolean;
  /** Ids of async sources that failed for the current query. */
  readonly failedSources: readonly string[];
  readonly entries: readonly PaletteEntry[];
  readonly sections: readonly PaletteSection[];
}

/**
 * Runs the registered palette sources for a live query: in-memory sources on
 * every keystroke, async ones debounced and cancelled as the query moves on.
 * Sources are never called before the Quran catalogue has loaded, so none of
 * them has to defend against a missing catalogue.
 */
export function createPaletteEngine(input: PaletteEngineInput): PaletteEngine {
  let quranData = $state<QuranData | null>(null);
  let asyncEntries = $state<PaletteEntry[]>([]);
  let searching = $state(false);
  let failedSources = $state<string[]>([]);
  let catalogueFailed = $state(false);

  const parsed = $derived(parseQuery(input.query()));
  const paletteQuery = $derived.by<PaletteQuery | null>(() => {
    const data = quranData;
    if (!data) return null;
    return {
      parsed,
      routeContext: input.routeContext(),
      quranData: data,
      limit: DEFAULT_SOURCE_LIMIT,
    };
  });

  const syncEntries = $derived(paletteQuery ? collectSyncEntries(paletteQuery) : []);
  const entries = $derived(dedupeEntries([...syncEntries, ...asyncEntries]));
  const sections = $derived(sectionsFor(entries));

  // The catalogue is small and cached process-wide; fetch it the first time the
  // palette opens rather than on every page load.
  $effect(() => {
    if (!input.open() || quranData) return;
    // Retried on the next open, since `loadQuranData` drops its cached failure.
    catalogueFailed = false;
    void loadQuranData()
      .then((data) => {
        quranData = data;
      })
      .catch((error: unknown) => {
        catalogueFailed = true;
        console.warn("[palette] catalogue unavailable:", error);
      });
  });

  $effect(() => {
    const query = paletteQuery;
    const reset = (): void => {
      asyncEntries = [];
      searching = false;
      failedSources = [];
    };

    if (!input.open() || !query || asyncSources(query).length === 0) {
      reset();
      return;
    }

    // Clear immediately so results from the previous query never sit under a
    // newer one while the debounce is pending.
    reset();
    searching = true;

    const controller = new AbortController();
    const timer = setTimeout(() => {
      void collectAsyncEntries(query, controller.signal)
        .then((result) => {
          if (controller.signal.aborted) return;
          asyncEntries = result.entries;
          failedSources = result.failed;
        })
        .finally(() => {
          if (!controller.signal.aborted) searching = false;
        });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      controller.abort();
      clearTimeout(timer);
      searching = false;
    };
  });

  return {
    get ready() {
      return quranData !== null;
    },
    get catalogueFailed() {
      return catalogueFailed;
    },
    get searching() {
      return searching;
    },
    get failedSources() {
      return failedSources;
    },
    get entries() {
      return entries;
    },
    get sections() {
      return sections;
    },
  };
}
