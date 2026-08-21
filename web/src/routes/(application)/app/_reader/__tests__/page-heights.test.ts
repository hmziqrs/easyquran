import { reader } from "$lib/stores/reader.svelte";
import { describe, expect, it } from "vite-plus/test";

import { PageHeightCache } from "../page-heights";

describe("PageHeightCache typography key", () => {
  it("namespaces measured heights by every typography axis", () => {
    const cache = new PageHeightCache();
    cache.save(3, 1234, 800);
    expect(cache.get(3, 800)).toBe(1234);
    expect(cache.get(3, 800)).toBe(1234);

    reader.setArabicFont("scheherazade-new");
    expect(cache.get(3, 800)).not.toBe(1234);
    reader.setArabicFont("amiri");
    expect(cache.get(3, 800)).toBe(1234);

    reader.growTranslation();
    expect(cache.get(3, 800)).not.toBe(1234);
    reader.shrinkTranslation();
    expect(cache.get(3, 800)).toBe(1234);

    reader.setTranslationFamily("serif");
    expect(cache.get(3, 800)).not.toBe(1234);
    reader.setTranslationFamily("sans");
    expect(cache.get(3, 800)).toBe(1234);

    reader.bigger();
    expect(cache.get(3, 800)).not.toBe(1234);
    reader.smaller();
    expect(cache.get(3, 800)).toBe(1234);
  });
});
