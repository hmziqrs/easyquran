import { clamp } from "es-toolkit";

export const SURAH_PAGE_WINDOW_SIZE = 5;

export function virtualPageWindow(
  pages: readonly number[],
  focus: number,
  size = SURAH_PAGE_WINDOW_SIZE,
): readonly number[] {
  if (pages.length <= size) return pages;
  const focusIndex = pages.reduce(
    (closest, page, index) =>
      Math.abs(page - focus) < Math.abs(pages[closest]! - focus) ? index : closest,
    0,
  );
  const before = Math.floor(size / 2);
  const start = clamp(focusIndex - before, 0, pages.length - size);
  return pages.slice(start, start + size);
}
