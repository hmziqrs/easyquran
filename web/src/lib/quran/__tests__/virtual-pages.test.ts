import { describe, expect, it } from "vite-plus/test";
import { SURAH_PAGE_WINDOW_SIZE, virtualPageWindow } from "$lib/quran/virtual-pages";

describe("Surah page virtual window", () => {
  it("keeps all pages until the bounded window is exceeded", () => {
    expect(virtualPageWindow([7, 8, 9], 9)).toEqual([7, 8, 9]);
  });

  it("keeps a five-page window around the visible page", () => {
    const pages = [7, 8, 9, 10, 11, 12, 13];
    expect(SURAH_PAGE_WINDOW_SIZE).toBe(5);
    expect(virtualPageWindow(pages, 7)).toEqual([7, 8, 9, 10, 11]);
    expect(virtualPageWindow(pages, 10)).toEqual([8, 9, 10, 11, 12]);
    expect(virtualPageWindow(pages, 13)).toEqual([9, 10, 11, 12, 13]);
  });

  it("uses the closest loaded page when the focus is outside the cache", () => {
    expect(virtualPageWindow([3, 4, 5, 6, 7, 8], 20)).toEqual([4, 5, 6, 7, 8]);
  });
});
