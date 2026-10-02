import { describe, expect, it } from "vite-plus/test";

import { READING_QUICK_MAX, readingFlowId, readingQuickPicks } from "../reading-flow";

describe("readingQuickPicks", () => {
  it("offers recent picks, then stacked translations, deduped", () => {
    expect(readingQuickPicks(null, ["ur.jalandhry", "en.pickthall"], ["en.sahih", "ur.jalandhry"])).toEqual([
      "ur.jalandhry",
      "en.pickthall",
      "en.sahih",
    ]);
  });

  it("puts a translation page's own translation first", () => {
    expect(readingQuickPicks("en.sahih", ["ur.jalandhry"], ["en.sahih"])).toEqual([
      "en.sahih",
      "ur.jalandhry",
    ]);
  });

  it("caps the chips", () => {
    const many = Array.from({ length: 12 }, (_, i) => `xx.t${i}`);
    expect(readingQuickPicks(null, many, [])).toHaveLength(READING_QUICK_MAX);
  });
});

describe("readingFlowId", () => {
  const stacked = ["en.sahih", "ur.jalandhry"];

  it("flows the Arabic on an Arabic page while Arabic is chosen", () => {
    expect(readingFlowId("arabic", "ur.jalandhry", null, null, stacked)).toBeNull();
  });

  it("flows the saved translation — stacked or not", () => {
    expect(readingFlowId("translation", "ur.jalandhry", null, null, stacked)).toBe("ur.jalandhry");
    expect(readingFlowId("translation", "fr.hamidullah", null, null, stacked)).toBe("fr.hamidullah");
  });

  it("falls back to the first stacked translation when nothing was picked yet", () => {
    expect(readingFlowId("translation", null, null, null, stacked)).toBe("en.sahih");
  });

  it("falls back to the Arabic when there is nothing to read", () => {
    expect(readingFlowId("translation", null, null, null, [])).toBeNull();
  });

  it("always flows a translation on a translation page — the route's own by default", () => {
    expect(readingFlowId("arabic", null, "en.sahih", null, stacked)).toBe("en.sahih");
  });

  it("honours a pick made on the translation page", () => {
    expect(readingFlowId("translation", null, "en.sahih", "fr.hamidullah", stacked)).toBe(
      "fr.hamidullah",
    );
  });
});
