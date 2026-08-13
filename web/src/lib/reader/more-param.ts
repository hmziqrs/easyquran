import { uniq } from "es-toolkit";

export const READER_MORE_PARAM = "more";

export function parseMoreParam(url: URL): string[] {
  const value = url.searchParams.get(READER_MORE_PARAM);
  if (!value) return [];
  return uniq(
    value
      .split(",")
      .map((part) => part.trim())
      .filter((part) => part.length > 0),
  );
}

export function withMoreParam(
  url: URL | string,
  ids: readonly string[],
  base?: URL | string,
): URL {
  const next = url instanceof URL ? url : new URL(url, base);
  if (ids.length > 0) {
    next.searchParams.set(READER_MORE_PARAM, uniq(ids).join(","));
  } else {
    next.searchParams.delete(READER_MORE_PARAM);
  }
  return next;
}

export function moreParamMatches(url: URL, ids: readonly string[]): boolean {
  const parsed = parseMoreParam(url);
  if (parsed.length !== ids.length) return false;
  const seen = new Set(parsed);
  for (const id of ids) if (!seen.has(id)) return false;
  return true;
}
