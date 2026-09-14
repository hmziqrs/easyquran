import {
  SURAH_PAGE_WINDOW_MAX,
  SURAH_PAGE_WINDOW_SIZE,
  virtualPageWindow,
  windowSizeForViewport,
} from "$lib/quran/virtual-pages";
import { describe, expect, it } from "vite-plus/test";

describe("Surah page virtual window", () => {
  it("keeps all pages until the bounded window is exceeded", () => {
    expect(virtualPageWindow([7, 8, 9], 9)).toEqual([7, 8, 9]);
  });

  it("keeps a seven-page window around the visible page", () => {
    const pages = [5, 6, 7, 8, 9, 10, 11, 12, 13];
    expect(SURAH_PAGE_WINDOW_SIZE).toBe(7);
    expect(virtualPageWindow(pages, 5)).toEqual([5, 6, 7, 8, 9, 10, 11]);
    expect(virtualPageWindow(pages, 9)).toEqual([6, 7, 8, 9, 10, 11, 12]);
    expect(virtualPageWindow(pages, 13)).toEqual([7, 8, 9, 10, 11, 12, 13]);
  });

  it("renders at least three hot pages on each side of the focus", () => {
    // The user-facing contract: +2/+3 and -2/-3 pages around focus are hot.
    const pages = Array.from({ length: 15 }, (_, i) => i + 1);
    const window = virtualPageWindow(pages, 8);
    expect(window).toEqual([5, 6, 7, 8, 9, 10, 11]);
    expect(window).toContain(8 - 3);
    expect(window).toContain(8 + 3);
  });

  it("uses the closest loaded page when the focus is outside the cache", () => {
    expect(virtualPageWindow([3, 4, 5, 6, 7, 8], 20)).toEqual([3, 4, 5, 6, 7, 8]);
  });
});

describe("viewport-adaptive window size", () => {
  it("falls back to the fixed minimum without a measurable viewport or page", () => {
    expect(windowSizeForViewport(0, 900)).toBe(SURAH_PAGE_WINDOW_SIZE);
    expect(windowSizeForViewport(900, 0)).toBe(SURAH_PAGE_WINDOW_SIZE);
    expect(windowSizeForViewport(900, Number.NaN)).toBe(SURAH_PAGE_WINDOW_SIZE);
  });

  it("keeps the minimum when a page is taller than the viewport", () => {
    // ~2000px page at a large Arabic size: half a viewport per side is one page.
    expect(windowSizeForViewport(900, 2000)).toBe(SURAH_PAGE_WINDOW_SIZE);
  });

  it("is at least the ±3 hot-page minimum as soon as a viewport is known", () => {
    // The minimum applies before any page is measured: the window is immediate,
    // never gated behind ResizeObserver heightCache entries.
    expect(SURAH_PAGE_WINDOW_SIZE).toBeGreaterThanOrEqual(7);
    expect(windowSizeForViewport(900, 600)).toBe(7);
  });

  it("grows (odd) as pages get shorter than the viewport", () => {
    // 900px viewport, ~450px pages: 1.5 viewports per side = 3 pages/side -> 7.
    expect(windowSizeForViewport(900, 450)).toBe(7);
    // ~300px pages: 5 per side -> 11.
    expect(windowSizeForViewport(900, 300)).toBe(11);
  });

  it("scales up on tall screens", () => {
    // 1440px viewport with ~400px pages: 6 per side -> 13.
    expect(windowSizeForViewport(1440, 400)).toBe(13);
  });

  it("clamps to the maximum window", () => {
    const size = windowSizeForViewport(900, 40);
    expect(size).toBe(SURAH_PAGE_WINDOW_MAX);
    expect(size % 2).toBe(1);
  });
});
