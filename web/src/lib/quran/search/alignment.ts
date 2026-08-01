import { normalizeArabicWithMap, type NormalizedArabicMap } from "./normalize.ts";

export interface SearchTextAlignment {
  matchNorm: string;
  matchDisplayStarts: number[];
  matchDisplayEnds: number[];
}

interface TextRange {
  start: number;
  end: number;
}

function wordRanges(text: string): TextRange[] {
  const ranges: TextRange[] = [];
  let start = 0;
  while (start < text.length) {
    const space = text.indexOf(" ", start);
    const end = space < 0 ? text.length : space;
    ranges.push({ start, end });
    start = end + 1;
  }
  return ranges;
}

/** Align one normalized range while retaining a monotonic position map. */
function alignRange(
  match: string,
  matchRange: TextRange,
  display: string,
  displayRange: TextRange,
  alignedDisplay: (number | undefined)[],
): void {
  const matchLength = matchRange.end - matchRange.start;
  const displayLength = displayRange.end - displayRange.start;
  const width = displayLength + 1;
  const costs = new Uint16Array((matchLength + 1) * width);

  for (let i = 1; i <= matchLength; i++) costs[i * width] = i;
  for (let j = 1; j <= displayLength; j++) costs[j] = j;
  for (let i = 1; i <= matchLength; i++) {
    for (let j = 1; j <= displayLength; j++) {
      const equal = match[matchRange.start + i - 1] === display[displayRange.start + j - 1];
      const diagonal = costs[(i - 1) * width + j - 1] + (equal ? 0 : 1);
      const deleted = costs[(i - 1) * width + j] + 1;
      const inserted = costs[i * width + j - 1] + 1;
      costs[i * width + j] = Math.min(diagonal, deleted, inserted);
    }
  }

  let i = matchLength;
  let j = displayLength;
  while (i > 0 || j > 0) {
    const current = costs[i * width + j];
    const equal =
      i > 0 && j > 0 && match[matchRange.start + i - 1] === display[displayRange.start + j - 1];
    if (equal && current === costs[(i - 1) * width + j - 1]) {
      alignedDisplay[matchRange.start + i - 1] = displayRange.start + j - 1;
      i--;
      j--;
    } else if (i > 0 && current === costs[(i - 1) * width + j] + 1) {
      i--;
    } else if (j > 0 && current === costs[i * width + j - 1] + 1) {
      j--;
    } else {
      alignedDisplay[matchRange.start + i - 1] = displayRange.start + j - 1;
      i--;
      j--;
    }
  }
}

function alignMatchToDisplay(
  match: NormalizedArabicMap,
  display: NormalizedArabicMap,
): { starts: number[]; ends: number[] } {
  const alignedDisplay: (number | undefined)[] = Array(match.normalized.length);
  const matchWords = wordRanges(match.normalized);
  const displayWords = wordRanges(display.normalized);

  if (matchWords.length === displayWords.length) {
    for (let word = 0; word < matchWords.length; word++) {
      alignRange(
        match.normalized,
        matchWords[word]!,
        display.normalized,
        displayWords[word]!,
        alignedDisplay,
      );
      if (word < matchWords.length - 1) {
        alignedDisplay[matchWords[word]!.end] = displayWords[word]!.end;
      }
    }
  } else {
    alignRange(
      match.normalized,
      { start: 0, end: match.normalized.length },
      display.normalized,
      { start: 0, end: display.normalized.length },
      alignedDisplay,
    );
  }

  const previous: (number | undefined)[] = Array(alignedDisplay.length);
  const next: (number | undefined)[] = Array(alignedDisplay.length);
  let nearest: number | undefined;
  for (let index = 0; index < alignedDisplay.length; index++) {
    previous[index] = nearest;
    nearest = alignedDisplay[index] ?? nearest;
  }
  nearest = undefined;
  for (let index = alignedDisplay.length - 1; index >= 0; index--) {
    next[index] = nearest;
    nearest = alignedDisplay[index] ?? nearest;
  }

  const starts: number[] = [];
  const ends: number[] = [];
  for (let index = 0; index < alignedDisplay.length; index++) {
    const displayIndex = alignedDisplay[index];
    if (displayIndex !== undefined) {
      starts.push(display.starts[displayIndex]!);
      ends.push(display.ends[displayIndex]!);
      continue;
    }

    const before = previous[index];
    const after = next[index];
    const boundary =
      after !== undefined
        ? display.starts[after]!
        : before !== undefined
          ? display.ends[before]!
          : 0;
    starts.push(boundary);
    ends.push(boundary);
  }
  return { starts, ends };
}

/**
 * Map normalized positions from the matching source to exact UTF-16 spans in
 * the display source. The result is built once and shared by SSG and Worker
 * search through the canonical corpus.
 */
export function alignSearchText(matchText: string, displayText: string): SearchTextAlignment {
  const match = normalizeArabicWithMap(matchText);
  const display = normalizeArabicWithMap(displayText);
  const aligned = alignMatchToDisplay(match, display);
  return {
    matchNorm: match.normalized,
    matchDisplayStarts: aligned.starts,
    matchDisplayEnds: aligned.ends,
  };
}
