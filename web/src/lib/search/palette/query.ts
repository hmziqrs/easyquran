import { normalizeArabic } from "$lib/quran/search/normalize";
import type { ParsedQuery } from "./types";

const LEADING_WORD = /^([\p{L}][\p{L}'’-]*)/u;
const TRAILING_REF = /(\d{1,4})(?:\s*[:.\-\s]\s*(\d{1,4}))?\s*$/;

/**
 * Tokenizes a raw palette query once for all sources. Deliberately dumb about
 * meaning: it reports the leading word and any trailing numeric reference, and
 * leaves interpretation to each source, so `bukhari 1:2` needs no change here.
 */
export function parseQuery(raw: string): ParsedQuery {
  const text = raw.trim().replace(/\s+/g, " ");
  const lower = text.toLowerCase();
  const word = LEADING_WORD.exec(text)?.[1];
  const keyword = word ? word.toLowerCase() : null;
  const afterKeyword = word ? text.slice(word.length).trim() : text;

  const ref = TRAILING_REF.exec(text);
  const numbers: number[] = [];
  if (ref) {
    numbers.push(Number(ref[1]));
    if (ref[2] !== undefined) numbers.push(Number(ref[2]));
  }

  return {
    text,
    lower,
    keyword,
    afterKeyword,
    numbers: Object.freeze(numbers),
    arabic: normalizeArabic(text),
    isEmpty: text.length === 0,
  };
}

/** True when the query's leading word is one of a source's aliases. */
export function hasKeyword(parsed: ParsedQuery, aliases: readonly string[]): boolean {
  return parsed.keyword !== null && aliases.includes(parsed.keyword);
}

/**
 * The query with a recognized keyword stripped, for name matching — `sura
 * baqarah` and `baqarah` should both rank Al-Baqarah the same way.
 */
export function termsFor(parsed: ParsedQuery, aliases: readonly string[]): string {
  return hasKeyword(parsed, aliases) ? parsed.afterKeyword : parsed.text;
}

/** Same as `termsFor`, normalized for Arabic comparison. */
export function arabicTermsFor(parsed: ParsedQuery, aliases: readonly string[]): string {
  return hasKeyword(parsed, aliases) ? normalizeArabic(parsed.afterKeyword) : parsed.arabic;
}

/**
 * Drops a trailing numeric reference so a name can be matched on its own —
 * `baqarah 255` and `البقرة 255` both leave just the name behind.
 */
export function stripTrailingRef(text: string): string {
  return text.replace(TRAILING_REF, "").trim();
}

/** Numeric reference with the parts named, when the query carries one. */
export function referenceNumbers(
  parsed: ParsedQuery,
): { primary: number; secondary?: number } | null {
  const [primary, secondary] = parsed.numbers;
  if (primary === undefined) return null;
  return secondary === undefined ? { primary } : { primary, secondary };
}

/** True when the whole query is just a number — ambiguous between domains. */
export function isBareNumber(parsed: ParsedQuery): boolean {
  return /^\d{1,4}$/.test(parsed.text);
}
