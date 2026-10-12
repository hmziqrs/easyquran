/**
 * Env-free leaf for derive-only text-kind rules shared by the reader view
 * (VerseRow/ReadingAyah/TranslationModal) and the server route parser. It must
 * never import site config (or anything else that reaches $app/env), so
 * component test graphs stay free of SvelteKit env modules — that is why it
 * lives beside the catalogue instead of inside it.
 */

/**
 * Tafsir works filed among the Arabic "translations" (TRX-01): commentary,
 * never a reading translation. Frozen id allow-list — the catalogue itself
 * never changes (derive-only rule); every id maps to a baked row.
 */
const TAFSIR_TRANSLATION_IDS: ReadonlySet<string> = new Set(["ar.jalalayn", "ar.muyassar"]);

/** True for catalogue ids that are commentary (tafsir), not translations. */
export function isTafsirTranslation(id: string): boolean {
  return TAFSIR_TRANSLATION_IDS.has(id);
}

/** One parsed run of a transliteration verse: the text plus its marked letters. */
export interface TransliterationSegment {
  readonly text: string;
  /** `<u>` run — a long vowel the reader should stretch. */
  readonly underline: boolean;
  /** `<b>` run — a silent/assimilated letter. */
  readonly strong: boolean;
}

const TRANSLIT_TAG = /<(\/?)([uUbB])>/gu;

/**
 * Tanzil transliterations mark long vowels (`<u>a</U>`) and assimilated
 * letters (`<b>l</B>`) with HTML-ish tags and case-insensitive closers. The
 * source data is immutable (repo rule), so the marks are parsed in the view
 * layer, like Tajweed markup: underline = stretch the vowel, bold =
 * silent/assimilated letter (TRX-02). Tags never nest in the data; any other
 * character (including a stray `<`) passes through verbatim.
 */
export function parseTransliterationSegments(text: string): readonly TransliterationSegment[] {
  if (!text.includes("<")) return [{ text, underline: false, strong: false }];
  const segments: TransliterationSegment[] = [];
  let underline = false;
  let strong = false;
  let cursor = 0;
  for (const match of text.matchAll(TRANSLIT_TAG)) {
    const at = match.index ?? cursor;
    const chunk = text.slice(cursor, at);
    if (chunk.length > 0) segments.push({ text: chunk, underline, strong });
    cursor = at + match[0]!.length;
    const closing = match[1] === "/";
    if (match[2] === "u" || match[2] === "U") underline = !closing;
    else strong = !closing;
  }
  const tail = text.slice(cursor);
  if (tail.length > 0) segments.push({ text: tail, underline, strong });
  return segments;
}

/** Copy/share text with the transliteration markup stripped (TRX-02). */
export function stripTransliterationMarkup(text: string): string {
  return text.replaceAll(TRANSLIT_TAG, "");
}
