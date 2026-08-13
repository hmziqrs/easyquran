import { untrack } from "svelte";
import type {
  StackedSourceState,
  StackedTranslation,
  StackedTranslationsByVerse,
  TranslationCatalogueEntry,
  VerseKey,
} from "$lib/data/quran-types";
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
    quranWorker
      .readRange(fromVal, toVal, validator, id)
      .then((range) => {
        if (disposed || startGen !== gen) return;
        if (!metaFor(id)) {
          setStatus(id, "error");
          return;
        }
        mergeRange(id, range.ayahs);
        setStatus(id, "ready");
      })
      .catch(() => {
        if (disposed || startGen !== gen) return;
        setStatus(id, "error");
      });
  }

  function sync(): void {
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
        for (const id of next.keys()) {
          if (!extras.includes(id)) next.delete(id);
        }
        return next;
      });
      status = pruned;
    }

    if (route !== prevRoute) {
      prevRoute = route;
      gen++;
      byVerse = new Map();
      status = new Map();
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
