import type { ReadonlyURL } from "$app/state";
import { uniq } from "es-toolkit";

export const READER_MORE_PARAM = "more";

export function parseMoreParam(url: URL | ReadonlyURL): string[] {
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
  url: URL | ReadonlyURL | string,
  ids: readonly string[],
  base?: URL | ReadonlyURL | string,
): URL {
  // kit 3 types page.url as a ReadonlyURL, and the readonly input must never be
  // mutated in place, so always clone (toString() is absolute for URL inputs,
  // leaving `base` to matter only for relative string inputs, as before).
  const next = new URL(url.toString(), base?.toString());
  if (ids.length > 0) {
    next.searchParams.set(READER_MORE_PARAM, uniq(ids).join(","));
  } else {
    next.searchParams.delete(READER_MORE_PARAM);
  }
  return next;
}

export function moreParamMatches(url: URL | ReadonlyURL, ids: readonly string[]): boolean {
  const parsed = parseMoreParam(url);
  if (parsed.length !== ids.length) return false;
  const seen = new Set(parsed);
  for (const id of ids) if (!seen.has(id)) return false;
  return true;
}
