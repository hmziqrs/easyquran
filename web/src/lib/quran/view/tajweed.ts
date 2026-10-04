import { QuranScript, type QuranScript as QuranScriptValue } from "../../data/quran-types.ts";

/**
 * Tajweed markup view (docs/quran-system.md — ayah text stays verbatim).
 *
 * The tajweed mushaf DB carries inline rule segments in `text`, byte-for-byte:
 * `[h:1468[ٱ]` — `[` + single rule letter + optional `:<id>` + `[` + colored
 * run + `]`. Groups may nest one level deep (33 verses, e.g. 2:190's
 * `[o[ُوٓ[s[اْ]‌ۚ]`): an inner rule group keeps its own color while the outer
 * body text around it keeps the outer rule. Rendering and plain-text views
 * parse it HERE only; the stored text, the API wire format, and every
 * non-tajweed source are untouched.
 */

export type TajweedRuleLetter =
  | "h"
  | "s"
  | "l"
  | "n"
  | "p"
  | "m"
  | "q"
  | "o"
  | "c"
  | "f"
  | "w"
  | "i"
  | "a"
  | "u"
  | "d"
  | "b"
  | "g";

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
  b: "#a1a1a1", // idgham mutaqaribayn (e.g. قُل رَّ, نَخْلُقْكُم)
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
    while (digitEnd < text.length && text[digitEnd]! >= "0" && text[digitEnd]! <= "9")
      digitEnd += 1;
    if (digitEnd === digitStart || text[digitEnd] !== "[") return null;
    bodyStart = digitEnd + 1;
  } else if (text[bodyStart] === "[") {
    bodyStart += 1;
  } else {
    return null;
  }
  return { rule: letter, bodyStart };
}

/**
 * Bare bracket group: `[` + Arabic-only content + `]` with no rule letter —
 * the corpus has exactly one (32:3 `فْتَرَ[ٮٰ]هُ`). The content is Quranic text
 * the edition bracketed without a rule, so it must render and only the
 * brackets drop. Non-Arabic or mixed content (a literal `[2]` footnote) is
 * not a bare group and keeps its brackets.
 */
// \u200D (ZWJ) must stay the LAST class member: the linter reads a ZWJ followed by
// another member as a joined character sequence; member order is otherwise irrelevant.
const BARE_ARABIC_GROUP =
  /^[\u0600-\u06FF\u0750-\u077F\u08A0-\u08FF\u200C\uFD70-\uFDFF\uFE70-\uFEFF\u200D]+$/u;

/** Whether `[content]` at `open` is a letterless Arabic bracket group. */
function isBareArabicGroup(text: string, open: number, close: number): boolean {
  if (close === -1 || close === open + 1) return false;
  const body = text.slice(open + 1, close);
  return !body.includes("[") && BARE_ARABIC_GROUP.test(body);
}

/** Merge adjacent plain runs so literal brackets never split text. */
function coalescePlain(segments: readonly TajweedSegment[]): readonly TajweedSegment[] {
  const out: TajweedSegment[] = [];
  for (const segment of segments) {
    const last = out[out.length - 1];
    if (segment.rule === null && last?.rule === null)
      out[out.length - 1] = { text: last.text + segment.text, rule: null };
    else out.push(segment);
  }
  return out;
}

/**
 * Parse runs from `cursor.index` (bracket-depth stack):
 * - top level (`groupRule === null`): plain text, group bodies, literal brackets;
 * - inside a group (`groupRule` set): body text carries the group's rule, inner
 *   rule groups recurse with their own rule (inner colors win inside outer
 *   runs), and the `]` closing THIS group ends the scan. A group left unclosed
 *   at end-of-string keeps its parsed body — markup never drops Quranic text.
 */
function parseBody(
  text: string,
  cursor: ScanCursor,
  segments: TajweedSegment[],
  groupRule: TajweedRuleLetter | null,
): void {
  let plain = "";
  const flush = (): void => {
    if (plain !== "") {
      segments.push({ text: plain, rule: groupRule });
      plain = "";
    }
  };
  while (cursor.index < text.length) {
    const ch = text[cursor.index];
    if (ch !== "[" && ch !== "]") {
      plain += ch;
      cursor.index += 1;
      continue;
    }
    if (ch === "]") {
      if (groupRule === null) {
        // Literal close bracket outside any group: keep the byte.
        plain += "]";
        cursor.index += 1;
        continue;
      }
      flush();
      cursor.index += 1; // consume this group's closing bracket
      return;
    }
    const opener = takeOpener(text, cursor);
    if (opener === null) {
      const bareClose = text.indexOf("]", cursor.index + 1);
      if (isBareArabicGroup(text, cursor.index, bareClose)) {
        // Letterless bracket group: render the Arabic content, drop brackets.
        plain += text.slice(cursor.index + 1, bareClose);
        cursor.index = bareClose + 1;
        continue;
      }
      // Literal bracket, not markup: keep the byte.
      plain += "[";
      cursor.index += 1;
      continue;
    }
    if (opener.bodyStart >= text.length) {
      // Opener at end-of-string with no body: its bytes stay literal.
      plain += text.slice(cursor.index, opener.bodyStart);
      cursor.index = opener.bodyStart;
      continue;
    }
    flush();
    cursor.index = opener.bodyStart;
    parseBody(text, cursor, segments, opener.rule);
  }
  // End of string: unclosed group keeps its parsed body (no closing bracket).
  flush();
}

/** Parse tajweed markup into colored/plain runs. Non-tajweed text passes through as one run. */
export function parseTajweedSegments(text: string): readonly TajweedSegment[] {
  if (!text.includes("[")) return [{ text, rule: null }];
  const segments: TajweedSegment[] = [];
  parseBody(text, { index: 0 }, segments, null);
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
