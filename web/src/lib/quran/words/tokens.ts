/**
 * Tokenises one ayah for display. Every slice is an exact substring of the
 * input, and slices are contiguous, so joining them reproduces the input byte
 * for byte. Word numbers count word slices only (1-based); whitespace slices
 * carry `word: null`.
 */
export interface WordSlice {
  readonly text: string;
  readonly word: number | null;
  readonly start: number;
  readonly end: number;
}

const RUN = /\s+|\S+/gu;

export function sliceWords(text: string): readonly WordSlice[] {
  const slices: WordSlice[] = [];
  let word = 0;
  for (const match of text.matchAll(RUN)) {
    const value = match[0];
    const start = match.index ?? 0;
    const isSpace = /^\s/u.test(value);
    if (!isSpace) word += 1;
    slices.push({
      text: value,
      word: isSpace ? null : word,
      start,
      end: start + value.length,
    });
  }
  return slices;
}
