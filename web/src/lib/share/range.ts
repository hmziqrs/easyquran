/** Most ayahs one share may carry (the reader's range span cap). */
export const SHARE_SPAN_MAX = 50;

export type RangeProblem = "invalid" | "too-long";

/** Checks an inclusive ayah range inside one surah. Null means the range can be shared. */
export function rangeProblem(from: number, to: number, ayahCount: number): RangeProblem | null {
  if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to)) return "invalid";
  if (from < 1 || to > ayahCount || from > to) return "invalid";
  if (to - from + 1 > SHARE_SPAN_MAX) return "too-long";
  return null;
}
