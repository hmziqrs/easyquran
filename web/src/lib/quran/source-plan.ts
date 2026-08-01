import { QuranSourceId, type QuranSourceId as QuranSourceIdValue } from "../data/quran-types.ts";

export interface QuranSourcePlan {
  readonly reader: QuranSourceIdValue;
  readonly search: {
    readonly match: QuranSourceIdValue;
    readonly display: QuranSourceIdValue;
  };
}

export const DEFAULT_QURAN_SOURCE_PLAN: QuranSourcePlan = Object.freeze({
  reader: QuranSourceId.TanzilUthmani,
  search: Object.freeze({
    match: QuranSourceId.TanzilSimpleClean,
    display: QuranSourceId.TanzilUthmani,
  }),
});

export function plannedSourceIds(plan: QuranSourcePlan): QuranSourceIdValue[] {
  return [...new Set([plan.reader, plan.search.match, plan.search.display])];
}
