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

export const DEFAULT_QURAN_SOURCE_PLAN: ArabicQuranSourcePlan = Object.freeze({
  reader: QuranSourceId.TanzilUthmani,
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
