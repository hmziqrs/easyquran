import { normalizeArabic } from "#lib/quran/search/normalize.js";

export interface ParsedQuery {
  /** Trimmed, whitespace-collapsed. */
  text: string;
  lower: string;
  /** Lowercased leading word token, or `null` when the query starts with a digit. */
  keyword: string | null;
  /** `text` with the leading word token removed, trimmed. */
  afterKeyword: string;
  /**
   * Numbers from a trailing `2:255` / `2 255` / `255` reference, in order. A
   * range suffix adds its end — `2:255-286` yields 3 numbers, `2:255-2:286`
   * yields 4 (see `rangeNumbers`).
   */
  numbers: readonly number[];
  /** `text` normalized for Arabic matching (see `#lib/quran/search/normalize.js`). */
  arabic: string;
  isEmpty: boolean;
}

const LEADING_WORD = /^([\p{L}][\p{L}'’-]*)/u;

/**
 * Digits as readers actually encounter them in a mushaf: ASCII, Arabic-Indic
 * (`٠-٩`) and Extended Arabic-Indic / Persian (`۰-۹`). The reader renders verse
 * numbers in Arabic-Indic, so a pasted or Arabic-keyboard `٢:٢٥٥` has to parse
 * exactly like `2:255`.
 */
const DIGIT = "0-9٠-٩۰-۹";
// Trailing `S:A` ref (separator `:`, `.`, `-` or space) with an optional range
// suffix: `-286` borrows the start surah (`2:255-286`), `-2:286` names the end
// surah (`2:255-2:286`). A lone `-` separator stays a separator, so `2-255`
// still parses as surah 2, ayah 255 — only a dash after a complete ref reads
// as a range.
const TRAILING_REF = new RegExp(
  `([${DIGIT}]{1,4})(?:\\s*[:.\\-\\s]\\s*([${DIGIT}]{1,4}))?` +
    `(?:\\s*-\\s*([${DIGIT}]{1,4})(?:\\s*:\\s*([${DIGIT}]{1,4}))?)?\\s*$`,
);
const BARE_NUMBER = new RegExp(`^[${DIGIT}]{1,4}$`);

/** Folds Arabic-Indic and Persian digits onto ASCII so `Number()` can read them. */
export function foldDigits(text: string): string {
  return text.replace(/[٠-٩۰-۹]/g, (d) => {
    const code = d.codePointAt(0)!;
    const base = code >= 0x06f0 ? 0x06f0 : 0x0660;
    return String(code - base);
  });
}

/**
 * Tokenizes a raw query once for all consumers. Deliberately dumb about
 * meaning: it reports the leading word and any trailing numeric reference, and
 * leaves interpretation to each caller, so `bukhari 1:2` needs no change here.
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
    for (const group of ref.slice(1)) {
      if (group !== undefined) numbers.push(Number(foldDigits(group)));
    }
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

/** True when the query's leading word is one of a caller's aliases. */
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

/** An ayah range's endpoints, when the trailing ref carries a range suffix. */
export interface AyahRange {
  readonly startSurah: number;
  readonly startAyah: number;
  readonly endSurah: number;
  readonly endAyah: number;
}

/**
 * The range behind `2:255-286` (end borrows the start surah) or
 * `2:255-2:286` (end names its surah), or `null` when the query has no range
 * suffix. Bounds are the caller's to check — this layer only reports shapes.
 */
export function rangeNumbers(parsed: ParsedQuery): AyahRange | null {
  const [startSurah, startAyah, end, endSecond] = parsed.numbers;
  if (startSurah === undefined || startAyah === undefined || end === undefined) return null;
  // 3 numbers: the lone end is an ayah in the start surah. 4: surah + ayah.
  if (endSecond === undefined) {
    return { startSurah, startAyah, endSurah: startSurah, endAyah: end };
  }
  return { startSurah, startAyah, endSurah: end, endAyah: endSecond };
}

/** True when the whole query is just a number — ambiguous between domains. */
export function isBareNumber(parsed: ParsedQuery): boolean {
  return BARE_NUMBER.test(parsed.text);
}

/**
 * The free text left after removing a recognized keyword and a trailing numeric
 * reference. Empty means the query is nothing but a coordinate (`2:255`,
 * `juz 5`, `112`), which lets expensive full-text sources sit out a query they
 * could only answer with noise.
 */
export function residualText(parsed: ParsedQuery, aliases: readonly string[]): string {
  return stripTrailingRef(termsFor(parsed, aliases));
}
