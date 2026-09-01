import { QuranScript, type QuranScript as QuranScriptValue } from "../../data/quran-types.ts";

/**
 * Tajweed markup view (docs/quran-system.md — ayah text stays verbatim).
 *
 * The tajweed mushaf DB carries inline rule segments in `text`, byte-for-byte:
 * `[h:1468[ٱ]` — `[` + single rule letter + optional `:<id>` + `[` + colored
 * run + `]`. Rendering and plain-text views parse it HERE only; the stored
 * text, the API wire format, and every non-tajweed source are untouched.
 */

export type TajweedRuleLetter =
  | "h" | "s" | "l" | "n" | "p" | "m" | "q" | "o"
  | "c" | "f" | "w" | "i" | "a" | "u" | "d" | "g";

/**
 * Rule letter → display color. Letters are the Dar Al-Islam tajweed edition's
 * markup keys (same lineage as quran.com's tajweed parsing). Palette follows
 * the widely published tajweed-color convention; unknown letters render in the
 * ambient text color (never dropped).
 */
const RULE_COLORS: Readonly<Record<TajweedRuleLetter, string>> = Object.freeze({
  h: "#9e9e9e", // hamzat al-wasl
  s: "#9e9e9e", // silent (sukun on unpronounced letter)
  l: "#9e9e9e", // lam shamsiyyah
  n: "#537fff", // madda normal (2 counts)
  p: "#4050ff", // madda permissible (2/4/6)
  m: "#000ebc", // madda necessary (6)
  o: "#2144c1", // madda obligatory (4–5)
  q: "#dd0008", // qalqalah
  c: "#d500b7", // ikhfa shafawi
  f: "#9400a8", // ikhfa
  w: "#c20067", // idgham shafawi
  i: "#26bffd", // iqlab
  a: "#169200", // idgham with ghunnah
  u: "#169200", // idgham without ghunnah
  d: "#a1a1a1", // idgham mutajanisayn/mutamathilayn
  g: "#ff7e1e", // ghunnah
});

function isRuleLetter(value: string): value is TajweedRuleLetter {
  return value in RULE_COLORS;
}

export function tajweedRuleColor(rule: TajweedRuleLetter): string {
  return RULE_COLORS[rule];
}

export interface TajweedSegment {
  /** Verbatim text run (never empty for markup segments from the parser). */
  readonly text: string;
  /** Rule letter when the run is a tajweed segment; null for plain runs. */
  readonly rule: TajweedRuleLetter | null;
}

interface ScanCursor {
  index: number;
}

function takeRun(text: string, cursor: ScanCursor): string {
  const next = text.indexOf("[", cursor.index);
  if (next === -1) {
    const run = text.slice(cursor.index);
    cursor.index = text.length;
    return run;
  }
  const run = text.slice(cursor.index, next);
  cursor.index = next;
  return run;
}

/**
 * Try to read one `[x` or `[x:id[` markup opener at `cursor.index`.
 * Returns null (cursor untouched) when the bracket is literal text.
 */
function takeOpener(
  text: string,
  cursor: ScanCursor,
): { rule: TajweedRuleLetter; bodyStart: number } | null {
  // Precondition: text[cursor.index] === "[" (caller guarantees it).
  const letter = text[cursor.index + 1];
  if (letter === undefined || !isRuleLetter(letter)) return null;
  let bodyStart = cursor.index + 2;
  if (text[bodyStart] === ":") {
    const digitStart = bodyStart + 1;
    let digitEnd = digitStart;
    while (digitEnd < text.length && text[digitEnd]! >= "0" && text[digitEnd]! <= "9") digitEnd += 1;
    if (digitEnd === digitStart || text[digitEnd] !== "[") return null;
    bodyStart = digitEnd + 1;
  } else if (text[bodyStart] === "[") {
    bodyStart += 1;
  } else {
    return null;
  }
  return { rule: letter, bodyStart };
}

/** Merge adjacent plain runs so literal brackets never split text. */
function coalescePlain(segments: readonly TajweedSegment[]): readonly TajweedSegment[] {
  const out: TajweedSegment[] = [];
  for (const segment of segments) {
    const last = out[out.length - 1];
    if (segment.rule === null && last?.rule === null) out[out.length - 1] = { text: last.text + segment.text, rule: null };
    else out.push(segment);
  }
  return out;
}

/** Parse tajweed markup into colored/plain runs. Non-tajweed text passes through as one run. */
export function parseTajweedSegments(text: string): readonly TajweedSegment[] {
  if (!text.includes("[")) return [{ text, rule: null }];
  const segments: TajweedSegment[] = [];
  const cursor: ScanCursor = { index: 0 };
  while (cursor.index < text.length) {
    if (text[cursor.index] !== "[") {
      const run = takeRun(text, cursor);
      if (run) segments.push({ text: run, rule: null });
      continue;
    }
    const opener = takeOpener(text, cursor);
    if (!opener) {
      // Literal bracket, not markup: fold it into the trailing plain run.
      const last = segments[segments.length - 1];
      if (last && last.rule === null) segments[segments.length - 1] = { text: `${last.text}[`, rule: null };
      else segments.push({ text: "[", rule: null });
      cursor.index += 1;
      continue;
    }
    const close = text.indexOf("]", opener.bodyStart);
    if (close === -1 && opener.bodyStart === text.length) {
      // Opener at end-of-string with no body: emit the bracket literally.
      segments.push({ text: "[", rule: null });
      cursor.index += 1;
      continue;
    }
    const end = close === -1 ? text.length : close;
    const body = text.slice(opener.bodyStart, end);
    if (body) segments.push({ text: body, rule: opener.rule });
    cursor.index = close === -1 ? text.length : end + 1;
  }
  return coalescePlain(segments);
}

/**
 * Strip tajweed markup for PLAIN-TEXT views only (copy, share, sidebar
 * previews, title attributes). Never applied to stored/wire/display text.
 */
export function stripTajweedMarkup(text: string): string {
  if (!text.includes("[")) return text;
  let out = "";
  for (const segment of parseTajweedSegments(text)) out += segment.text;
  return out;
}

/** Whether a verse should render through the tajweed segment renderer. */
export function isTajweedScript(script: QuranScriptValue): boolean {
  return script === QuranScript.Tajweed;
}
