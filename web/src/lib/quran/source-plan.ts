import { uniq } from "es-toolkit";

import {
  QuranSourceId,
  type QuranReaderSource,
  type QuranSourceId as QuranSourceIdValue,
} from "../data/quran-types.ts";

export interface QuranSourcePlan {
  readonly reader: QuranReaderSource;
  readonly search: {
    readonly match: QuranSourceIdValue;
    readonly display: QuranSourceIdValue;
  };
}

type ArabicQuranSourcePlan = {
  readonly reader: QuranSourceIdValue;
  readonly search: {
    readonly match: QuranSourceIdValue;
    readonly display: QuranSourceIdValue;
  };
};

// Reader default = annotated Uthmani (SSR + worker preferred source). Search stays on the
// plain Tanzil Uthmani: the match corpus normalizes diacritics anyway, and the annotated
// marks (U+06E2/U+06ED, stop signs) would only add normalization noise there.
export const DEFAULT_QURAN_SOURCE_PLAN: ArabicQuranSourcePlan = Object.freeze({
  reader: QuranSourceId.AnnotatedUthmani,
  search: Object.freeze({
    match: QuranSourceId.TanzilUthmani,
    display: QuranSourceId.TanzilUthmani,
  }),
});

export function plannedSourceIds<P extends QuranSourcePlan>(
  plan: P,
): (P["reader"] | P["search"]["match"] | P["search"]["display"])[] {
  return uniq([plan.reader, plan.search.match, plan.search.display]);
}
