import { READER_MODE_VALUES, type ReaderMode } from "$lib/stores/reader-core.svelte";

export const READER_MODE_PARAM = "mode";

const VALID_MODES: ReadonlySet<string> = new Set(READER_MODE_VALUES);

export function parseModeParam(url: URL): ReaderMode | null {
  const value = url.searchParams.get(READER_MODE_PARAM);
  // SAFETY: VALID_MODES.has(value) proved value is one of READER_MODE_VALUES, the ReaderMode union.
  return value && VALID_MODES.has(value) ? (value as ReaderMode) : null;
}

export function withModeParam(url: URL | string, mode: ReaderMode, base?: URL | string): URL {
  const next = url instanceof URL ? url : new URL(url, base);
  next.searchParams.set(READER_MODE_PARAM, mode);
  return next;
}

export function modeParamMatches(url: URL, mode: ReaderMode): boolean {
  return url.searchParams.get(READER_MODE_PARAM) === mode;
}
