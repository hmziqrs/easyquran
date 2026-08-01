import type { Highlight } from "./types.ts";

export interface HighlightSegment {
  readonly start: number;
  readonly end: number;
  readonly text: string;
  readonly highlighted: boolean;
}

/**
 * Partition text using UTF-16 offsets from the search contract. Ranges are
 * normalized defensively so API data can never produce overlapping markup or
 * slice outside the exact string being rendered.
 */
export function highlightSegments(
  text: string,
  highlights: readonly Highlight[],
): HighlightSegment[] {
  const ranges = highlights
    .filter(
      ({ start, end }) =>
        Number.isSafeInteger(start) &&
        Number.isSafeInteger(end) &&
        start >= 0 &&
        end > start &&
        start < text.length,
    )
    .map(({ start, end }) => ({ start, end: Math.min(end, text.length) }))
    .sort((a, b) => a.start - b.start || a.end - b.end);

  const merged: Highlight[] = [];
  for (const range of ranges) {
    const previous = merged.at(-1);
    if (previous && range.start <= previous.end) {
      previous.end = Math.max(previous.end, range.end);
    } else {
      merged.push({ ...range });
    }
  }

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const range of merged) {
    if (cursor < range.start) {
      segments.push({
        start: cursor,
        end: range.start,
        text: text.slice(cursor, range.start),
        highlighted: false,
      });
    }
    segments.push({
      start: range.start,
      end: range.end,
      text: text.slice(range.start, range.end),
      highlighted: true,
    });
    cursor = range.end;
  }
  if (cursor < text.length) {
    segments.push({
      start: cursor,
      end: text.length,
      text: text.slice(cursor),
      highlighted: false,
    });
  }
  return segments;
}
