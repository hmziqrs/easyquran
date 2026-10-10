import {
  QURAN_SOURCE_IDS,
  QuranSourceId,
  type QuranSourceId as QuranSourceIdValue,
} from "#lib/data/quran-types.js";

import { QuranDataEnvironment } from "./environment.js";

/**
 * Production-gated by campaign decision: IndoPak and Tajweed stay referenced in
 * code deliberately; flip only when licence/policy clears.
 */
export const PRODUCTION_DISABLED_SOURCES: ReadonlySet<QuranSourceIdValue> = new Set([
  QuranSourceId.Indopak,
  QuranSourceId.Tajweed,
]);

export function isScriptEnabled(
  sourceId: QuranSourceIdValue,
  environment: QuranDataEnvironment,
): boolean {
  if (environment !== QuranDataEnvironment.Production) return true;
  return !PRODUCTION_DISABLED_SOURCES.has(sourceId);
}

export function enabledScriptIds(environment: QuranDataEnvironment): readonly QuranSourceIdValue[] {
  return QURAN_SOURCE_IDS.filter((sourceId) => isScriptEnabled(sourceId, environment));
}
