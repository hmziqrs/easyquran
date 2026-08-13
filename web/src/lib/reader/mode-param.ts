import { READER_MODE_VALUES, type ReaderMode } from "$lib/stores/reader-core.svelte";

export const READER_MODE_PARAM = "mode";

const VALID_MODES: ReadonlySet<string> = new Set(READER_MODE_VALUES);

export function parseModeParam(url: URL): ReaderMode | null {
  const value = url.searchParams.get(READER_MODE_PARAM);
  return value && VALID_MODES.has(value) ? (value as ReaderMode) : null;
}

export function withModeParam(url: URL | string, mode: ReaderMode, base?: URL | string): URL {
  const next = typeof url === "string" ? new URL(url, base) : new URL(url);
  next.searchParams.set(READER_MODE_PARAM, mode);
  return next;
}

export function modeParamMatches(url: URL, mode: ReaderMode): boolean {
  return url.searchParams.get(READER_MODE_PARAM) === mode;
}
