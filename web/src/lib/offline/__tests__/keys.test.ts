import { describe, expect, it } from "vite-plus/test";
import { normalizeDataKey } from "$lib/offline/keys";

describe("normalizeDataKey", () => {
  it("strips only the x-sveltekit-* reserved params", () => {
    expect(normalizeDataKey("https://h/app/al-kahf/__data.json?x-sveltekit-invalidated=01")).toBe(
      "/app/al-kahf/__data.json",
    );
  });

  it("preserves application query params (e.g. ?verse= deep links)", () => {
    expect(
      normalizeDataKey("https://h/app/al-kahf/__data.json?verse=50&x-sveltekit-invalidated=01"),
    ).toBe("/app/al-kahf/__data.json?verse=50");
  });

  it("strips the reader-only `mode` presentation param (identical HTML per mode)", () => {
    expect(normalizeDataKey("https://h/app/al-kahf/__data.json?mode=reading&verse=50")).toBe(
      "/app/al-kahf/__data.json?verse=50",
    );
    expect(normalizeDataKey("https://h/app/al-kahf/__data.json?mode=verse")).toBe(
      "/app/al-kahf/__data.json",
    );
  });

  it("preserves the pathname and a trailing slash", () => {
    expect(normalizeDataKey("https://h/app/juz/?x=1")).toBe("/app/juz/?x=1");
  });

  it("keeps unrelated params that merely start with x", () => {
    expect(normalizeDataKey("https://h/p?xfoo=1&x-sveltekit-invalidated=0")).toBe("/p?xfoo=1");
  });

  it("returns the bare pathname when every param is reserved", () => {
    expect(normalizeDataKey("https://h/p?x-sveltekit-invalidated=0_1&x-sveltekit-trailing=1")).toBe(
      "/p",
    );
  });

  it("accepts a URL object", () => {
    expect(normalizeDataKey(new URL("https://h/p?q=1&r=2"))).toBe("/p?q=1&r=2");
  });

  it("accepts a relative path (the pack's path-only route keys)", () => {
    expect(normalizeDataKey("/app/al-kahf/__data.json")).toBe("/app/al-kahf/__data.json");
  });
});
