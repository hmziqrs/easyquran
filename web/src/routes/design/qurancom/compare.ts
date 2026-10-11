/**
 * Pure helpers for the quran.com parity specimen (dev route + audit script).
 *
 * Identity is id + size; nothing here hashes (AGENTS.MD hard rule). This module is
 * dependency-pure: the web route and the root tsx audit both import it directly.
 */

import { stripTajweedMarkup } from "../../../lib/quran/view/tajweed.ts";
import { asArray, asObject, asString } from "../../../lib/storage/decoders.ts";

export type QurancomScriptSlug =
  | "uthmani"
  | "indopak"
  | "indopak_nastaleeq"
  | "imlaei"
  | "imlaei_simple"
  | "uthmani_tajweed";

export type MirrorFontKey = "mirror-hafs" | "mirror-nastaleeq" | "mirror-naskh";
export type OurFontKey = "ours-hafs" | "ours-indopak";

export interface QurancomOurDb {
  readonly id: string;
  readonly dbFile: string;
  readonly label: string;
}

export interface QurancomPair {
  readonly slug: QurancomScriptSlug;
  readonly label: string;
  readonly mirrorFile: string;
  readonly mirrorField: string;
  /** True when the pair compares markup-stripped bare text (tajweed formats differ by design). */
  readonly bare: boolean;
  readonly mirrorFont: MirrorFontKey;
  readonly ourFont: OurFontKey;
  readonly ours: readonly QurancomOurDb[];
}

export const QURANCOM_PAIRS: readonly QurancomPair[] = [
  {
    slug: "uthmani",
    label: "Uthmani (mirror text_uthmani)",
    mirrorFile: "uthmani.json",
    mirrorField: "text_uthmani",
    bare: false,
    mirrorFont: "mirror-hafs",
    ourFont: "ours-hafs",
    ours: [
      { id: "quran-uthmani", dbFile: "quran-uthmani.sqlite", label: "quran-uthmani" },
      {
        id: "quran-uthmani-annotated",
        dbFile: "quran-uthmani-annotated.sqlite",
        label: "quran-uthmani-annotated",
      },
    ],
  },
  {
    slug: "indopak",
    label: "IndoPak (mirror text_indopak)",
    mirrorFile: "indopak.json",
    mirrorField: "text_indopak",
    bare: false,
    mirrorFont: "mirror-nastaleeq",
    ourFont: "ours-indopak",
    ours: [{ id: "quran-indopak", dbFile: "quran-indopak.sqlite", label: "quran-indopak" }],
  },
  {
    slug: "indopak_nastaleeq",
    label: "IndoPak Nastaleeq (mirror text_indopak_nastaleeq)",
    mirrorFile: "indopak_nastaleeq.json",
    mirrorField: "text_indopak_nastaleeq",
    bare: false,
    mirrorFont: "mirror-nastaleeq",
    ourFont: "ours-indopak",
    ours: [
      {
        id: "quran-indopak-nastaleeq",
        dbFile: "quran-indopak-nastaleeq.sqlite",
        label: "quran-indopak-nastaleeq",
      },
    ],
  },
  {
    slug: "imlaei",
    label: "Imlaei (mirror text_imlaei)",
    mirrorFile: "imlaei.json",
    mirrorField: "text_imlaei",
    bare: false,
    mirrorFont: "mirror-naskh",
    ourFont: "ours-hafs",
    ours: [
      {
        id: "quran-simple-clean",
        dbFile: "quran-simple-clean.sqlite",
        label: "quran-simple-clean",
      },
    ],
  },
  {
    slug: "imlaei_simple",
    label: "Imlaei Simple (mirror text_imlaei_simple)",
    mirrorFile: "imlaei_simple.json",
    mirrorField: "text_imlaei_simple",
    bare: false,
    mirrorFont: "mirror-naskh",
    ourFont: "ours-hafs",
    ours: [
      {
        id: "quran-simple-clean",
        dbFile: "quran-simple-clean.sqlite",
        label: "quran-simple-clean",
      },
    ],
  },
  {
    slug: "uthmani_tajweed",
    label: "Uthmani Tajweed (mirror text_uthmani_tajweed)",
    mirrorFile: "uthmani_tajweed.json",
    mirrorField: "text_uthmani_tajweed",
    bare: true,
    mirrorFont: "mirror-hafs",
    ourFont: "ours-hafs",
    ours: [{ id: "quran-tajweed", dbFile: "quran-tajweed.sqlite", label: "quran-tajweed" }],
  },
];

export function pairBySlug(slug: string): QurancomPair | undefined {
  return QURANCOM_PAIRS.find((pair) => pair.slug === slug);
}

/**
 * Decode a raw quran.com API v4 script payload: { verses: [{ verse_key, text_* }] }.
 * Uses the shared JSON boundary decoders, so a malformed entry is skipped, never trusted.
 */
export function decodeMirrorVerses(jsonText: string, field: string): Map<string, string> {
  const verses = new Map<string, string>();
  const payload = asObject(JSON.parse(jsonText));
  if (!payload) return verses;
  for (const entry of asArray(payload.verses, asObject)) {
    const key = asString(entry.verse_key);
    const text = asString(entry[field]);
    if (key === undefined || text === undefined) continue;
    verses.set(key, text);
  }
  return verses;
}

const MIRROR_END_MARKER = /<span class=end>[^<]*<\/span>/gu;
const MIRROR_TAG = /<\/?tajweed[^>]*>/gu;

/** Strip quran.com's tajweed HTML to bare text (end ornaments drop with their markup). */
export function stripMirrorMarkup(html: string): string {
  return html.replace(MIRROR_END_MARKER, "").replace(MIRROR_TAG, "").trim();
}

export function mirrorBareText(pair: QurancomPair, raw: string): string {
  if (!pair.bare) return raw;
  return stripMirrorMarkup(raw);
}

export function ourBareText(pair: QurancomPair, raw: string): string {
  if (!pair.bare) return raw;
  return stripTajweedMarkup(raw).trim();
}

export interface DiffSummary {
  readonly equal: boolean;
  readonly diffCount: number;
  readonly firstIndex: number | null;
  readonly mirrorCodePoint: number | null;
  readonly oursCodePoint: number | null;
}

function codePointOf(value: string | undefined): number | null {
  if (value === undefined) return null;
  return value.codePointAt(0) ?? null;
}

/** Codepoint-aligned diff: first divergence index, codepoints there, and a full diff count. */
export function diffSummary(mirror: string, ours: string): DiffSummary {
  const a = Array.from(mirror);
  const b = Array.from(ours);
  const shared = Math.min(a.length, b.length);
  let diffCount = Math.abs(a.length - b.length);
  let firstIndex: number | null = null;
  for (let index = 0; index < shared; index += 1) {
    if (a[index] === b[index]) continue;
    diffCount += 1;
    if (firstIndex === null) firstIndex = index;
  }
  if (firstIndex === null && a.length !== b.length) firstIndex = shared;
  return {
    equal: firstIndex === null,
    diffCount,
    firstIndex,
    mirrorCodePoint: firstIndex === null ? null : codePointOf(a[firstIndex]),
    oursCodePoint: firstIndex === null ? null : codePointOf(b[firstIndex]),
  };
}

export function formatCodePoint(codePoint: number | null): string {
  if (codePoint === null) return "end-of-text";
  return `U+${codePoint.toString(16).toUpperCase().padStart(4, "0")}`;
}

export function describeDiff(summary: DiffSummary): string {
  if (summary.equal) return "equal";
  const index = summary.firstIndex ?? 0;
  const mirror = formatCodePoint(summary.mirrorCodePoint);
  const ours = formatCodePoint(summary.oursCodePoint);
  return `${summary.diffCount} diffs — first divergence at index ${index}: ${mirror} vs ${ours}`;
}

export type ParityBucket =
  | "bismillah-packaging"
  | "annotation-marks"
  | "tanween-encoding"
  | "maddah-encoding"
  | "hamza-encoding"
  | "waqf-mark-difference"
  | "direction-mark"
  | "spacing"
  | "vocalization"
  | "tajweed-markup"
  | "other";

const ANNOTATION_MARKS: ReadonlySet<number> = new Set([0x06e2, 0x06ed, 0x06e5, 0x06e6, 0x06e9]);
const WAQF_MARKS: ReadonlySet<number> = new Set([
  0x0615, 0x06d6, 0x06d7, 0x06d8, 0x06d9, 0x06da, 0x06db, 0x06dc, 0x06de,
]);
const TANWEEN_MARKS: ReadonlySet<number> = new Set([0x064b, 0x064c, 0x064d]);
const MADDAH_MARKS: ReadonlySet<number> = new Set([0x0622, 0x0653, 0x06e4]);
const HAMZA_LETTERS: ReadonlySet<number> = new Set([0x0621, 0x0623, 0x0624, 0x0625, 0x0626]);
const VOWEL_MARKS: ReadonlySet<number> = new Set([0x064e, 0x064f, 0x0650, 0x0651, 0x0652]);
const DIRECTION_MARKS: ReadonlySet<number> = new Set([0x200e, 0x200f]);
const SPACING: ReadonlySet<number> = new Set([0x0020, 0x00a0, 0x2002, 0x2003, 0x2009, 0x200b]);
const MARKUP_RESIDUE: ReadonlySet<number> = new Set([0x003c, 0x003e, 0x005b, 0x005d]);

const BISMILLAH = "بسم";
const ARABIC_MARKS = /[\u0640\u064b-\u0655\u0670\u06d6-\u06ed]/gu;

/** Bismillah prefix check after dropping diacritics (uthmani, indopak and imlaei spellings). */
export function startsWithBismillah(text: string): boolean {
  const head = Array.from(text).slice(0, 14).join("");
  return head.replace(ARABIC_MARKS, "").startsWith(BISMILLAH);
}

function isPrivateUse(codePoint: number): boolean {
  if (codePoint >= 0xe000 && codePoint <= 0xf8ff) return true;
  if (codePoint >= 0xf0000 && codePoint <= 0xffffd) return true;
  return codePoint >= 0x100000 && codePoint <= 0x10fffd;
}

function isAnnotationMark(codePoint: number): boolean {
  return ANNOTATION_MARKS.has(codePoint) || isPrivateUse(codePoint);
}

function isWaqfMark(codePoint: number): boolean {
  return WAQF_MARKS.has(codePoint);
}

function isTanween(codePoint: number): boolean {
  return TANWEEN_MARKS.has(codePoint);
}

function isMaddah(codePoint: number): boolean {
  return MADDAH_MARKS.has(codePoint);
}

function isHamza(codePoint: number): boolean {
  return HAMZA_LETTERS.has(codePoint);
}

function isVowel(codePoint: number): boolean {
  return VOWEL_MARKS.has(codePoint);
}

function isDirectionMark(codePoint: number): boolean {
  return DIRECTION_MARKS.has(codePoint);
}

function isSpacing(codePoint: number): boolean {
  return SPACING.has(codePoint);
}

function classifyTail(tail: readonly string[]): ParityBucket {
  const points = tail.map((value) => value.codePointAt(0) ?? 0);
  if (points.some((point) => isDirectionMark(point))) return "direction-mark";
  if (points.some((point) => isAnnotationMark(point))) return "annotation-marks";
  if (points.some((point) => isWaqfMark(point))) return "waqf-mark-difference";
  if (points.some((point) => isSpacing(point))) return "spacing";
  return "other";
}

export interface MismatchContext {
  readonly pair: QurancomPair;
  readonly mirrorRaw: string;
  readonly oursRaw: string;
  readonly mirrorBare: string;
  readonly oursBare: string;
}

/** Classify a mismatch by its first divergence (tail-only divergences classify by tail content). */
export function classifyMismatch(context: MismatchContext): ParityBucket {
  const { pair, mirrorRaw, oursRaw, mirrorBare, oursBare } = context;
  if (pair.bare && mirrorRaw !== oursRaw && mirrorBare === oursBare) return "tajweed-markup";
  const diff = diffSummary(mirrorBare, oursBare);
  if (diff.firstIndex === null) return "other";
  if (diff.firstIndex === 0 && (startsWithBismillah(mirrorBare) || startsWithBismillah(oursBare))) {
    return "bismillah-packaging";
  }
  const mirrorCp = diff.mirrorCodePoint;
  const oursCp = diff.oursCodePoint;
  if (mirrorCp === null || oursCp === null) {
    const tail = Array.from(mirrorCp === null ? oursBare : mirrorBare).slice(diff.firstIndex);
    return classifyTail(tail);
  }
  if (pair.bare && (MARKUP_RESIDUE.has(mirrorCp) || MARKUP_RESIDUE.has(oursCp))) {
    return "tajweed-markup";
  }
  if (isDirectionMark(mirrorCp) || isDirectionMark(oursCp)) return "direction-mark";
  if (isAnnotationMark(mirrorCp) || isAnnotationMark(oursCp)) return "annotation-marks";
  if (isSpacing(mirrorCp) || isSpacing(oursCp)) return "spacing";
  if (isMaddah(mirrorCp) || isMaddah(oursCp)) return "maddah-encoding";
  if (isHamza(mirrorCp) || isHamza(oursCp)) return "hamza-encoding";
  if (isTanween(mirrorCp) || isTanween(oursCp)) return "tanween-encoding";
  if (isWaqfMark(mirrorCp) || isWaqfMark(oursCp)) return "waqf-mark-difference";
  if (isVowel(mirrorCp) || isVowel(oursCp)) return "vocalization";
  return "other";
}
