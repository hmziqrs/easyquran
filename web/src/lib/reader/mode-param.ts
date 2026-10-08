import type { ReadonlyURL } from "$app/state";

import { READER_MODE_VALUES, type ReaderMode } from "#lib/stores/reader-core.svelte.js";

export const READER_MODE_PARAM = "mode";

const VALID_MODES: ReadonlySet<string> = new Set(READER_MODE_VALUES);

export function parseModeParam(url: URL | ReadonlyURL): ReaderMode | null {
  const value = url.searchParams.get(READER_MODE_PARAM);
  // SAFETY: VALID_MODES.has(value) proved value is one of READER_MODE_VALUES, the ReaderMode union.
  return value && VALID_MODES.has(value) ? (value as ReaderMode) : null;
}

export function withModeParam(
  url: URL | ReadonlyURL | string,
  mode: ReaderMode,
  base?: URL | ReadonlyURL | string,
): URL {
  // kit 3 types page.url as a ReadonlyURL, and the readonly input must never be
  // mutated in place, so always clone (toString() is absolute for URL inputs,
  // leaving `base` to matter only for relative string inputs, as before).
  const next = new URL(url.toString(), base?.toString());
  next.searchParams.set(READER_MODE_PARAM, mode);
  return next;
}

export function modeParamMatches(url: URL | ReadonlyURL, mode: ReaderMode): boolean {
  return url.searchParams.get(READER_MODE_PARAM) === mode;
}
