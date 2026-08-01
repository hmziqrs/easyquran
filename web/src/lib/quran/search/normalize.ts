export const SEARCH_VERSION = "arabic-search-v2";
export const MIN_QUERY_LEN = 3;
export const MAX_QUERY_LEN = 64;
export const DEFAULT_LIMIT = 20;
export const MAX_LIMIT = 50;
export const DEFAULT_OFFSET = 0;
export const MAX_OFFSET = 500;

const BARE_ALEF = "ا";
const YA = "ي";
const HA = "ه";
const REMOVED = /[\p{Mn}\p{Me}\p{Cf}\u0640]/u;

export interface NormalizedArabicMap {
  normalized: string;
  starts: number[];
  ends: number[];
}

export function normalizeArabicWithMap(input: string): NormalizedArabicMap {
  const values: { value: string; start: number; end: number }[] = [];
  let utf16 = 0;
  for (const scalar of input) {
    const start = utf16;
    utf16 += scalar.length;
    if (REMOVED.test(scalar)) {
      const previous = values.at(-1);
      if (previous) previous.end = utf16;
      continue;
    }

    let value = scalar;
    if (/[آأإٱ]/u.test(value)) value = BARE_ALEF;
    else if (value === "ى") value = YA;
    else if (value === "ة") value = HA;

    if (/\s/u.test(value)) {
      if (values.length === 0) continue;
      if (values.at(-1)?.value === " ") {
        values.at(-1)!.end = utf16;
        continue;
      }
      value = " ";
    }
    values.push({ value, start, end: utf16 });
  }
  if (values.at(-1)?.value === " ") values.pop();

  const starts: number[] = [];
  const ends: number[] = [];
  let normalized = "";
  for (const value of values) {
    normalized += value.value;
    for (let i = 0; i < value.value.length; i++) {
      starts.push(value.start);
      ends.push(value.end);
    }
  }
  return { normalized, starts, ends };
}

export function normalizeArabic(input: string): string {
  return normalizeArabicWithMap(input).normalized;
}

export function scalarLength(s: string): number {
  let n = 0;
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  for (const _ of s) n++;
  return n;
}

export function isEligibleQuery(norm: string): boolean {
  const len = scalarLength(norm);
  return len >= MIN_QUERY_LEN && len <= MAX_QUERY_LEN;
}
