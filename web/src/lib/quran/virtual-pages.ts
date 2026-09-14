import { clamp } from "es-toolkit";

// Minimum rendered window: focus page ±3 "hot pages" on each side, available
// immediately (SSR + first client render) with no height measurement. Larger
// sizes come from windowSizeForViewport once the focus page height is known.
export const SURAH_PAGE_WINDOW_SIZE = 7;
export const SURAH_PAGE_WINDOW_MAX = 15;

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

/**
 * How many pages to keep rendered around the focus page. Small Arabic sizes make
 * a mushaf page far shorter than one viewport, so a fixed window leaves the
 * reader staring at spacers (and the load-ahead firing constantly); scale the
 * window with the viewport against the focus page's measured (or estimated)
 * height, keeping an odd count so the focus page stays centred.
 */
export function windowSizeForViewport(
  viewportHeight: number,
  focusPageHeight: number,
  min = SURAH_PAGE_WINDOW_SIZE,
  max = SURAH_PAGE_WINDOW_MAX,
): number {
  if (viewportHeight <= 0 || !Number.isFinite(focusPageHeight) || focusPageHeight <= 0) return min;
  const perSide = Math.ceil((viewportHeight * 1.5) / focusPageHeight);
  return clamp(perSide * 2 + 1, min, max);
}
