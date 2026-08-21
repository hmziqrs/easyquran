import type { QuranRangeText } from "$lib/data/quran-types";
import {
  TranslationRangeCache,
  translationRangeCacheKey,
} from "$lib/server/translation-range-cache";
import { describe, expect, it, vi } from "vite-plus/test";

const RANGE = { ayahs: [], normalizations: [] } satisfies QuranRangeText;

describe("TranslationRangeCache", () => {
  it("shares concurrent loads and reuses the validated result", async () => {
    const cache = new TranslationRangeCache();
    const load = vi.fn(async () => RANGE);

    const [first, second] = await Promise.all([
      cache.getOrLoad("en.sahih:1-7", load),
      cache.getOrLoad("en.sahih:1-7", load),
    ]);
    const third = await cache.getOrLoad("en.sahih:1-7", load);

    expect(load).toHaveBeenCalledTimes(1);
    expect(first).toBe(RANGE);
    expect(second).toBe(RANGE);
    expect(third).toBe(RANGE);
  });

  it("never caches failed loads", async () => {
    const cache = new TranslationRangeCache();
    const load = vi
      .fn<() => Promise<QuranRangeText>>()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce(RANGE);

    await expect(cache.getOrLoad("en.sahih:1-7", load)).rejects.toThrow("offline");
    await expect(cache.getOrLoad("en.sahih:1-7", load)).resolves.toBe(RANGE);

    expect(load).toHaveBeenCalledTimes(2);
  });

  it("evicts least-recently-used values at its bound", async () => {
    const cache = new TranslationRangeCache(2);
    const load = vi.fn(async () => RANGE);

    await cache.getOrLoad("a", load);
    await cache.getOrLoad("b", load);
    await cache.getOrLoad("a", load);
    await cache.getOrLoad("c", load);
    await cache.getOrLoad("b", load);

    expect(cache.size).toBe(2);
    expect(load).toHaveBeenCalledTimes(4);
  });

  it("keys immutable content by source and exact canonical bounds", () => {
    expect(translationRangeCacheKey("en.sahih", 1, 7)).toBe("en.sahih:1-7");
  });
});
