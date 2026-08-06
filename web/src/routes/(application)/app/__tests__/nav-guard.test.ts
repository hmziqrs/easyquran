import { describe, expect, it } from "vite-plus/test";
import {
  globalPagePathFor,
  juzPathFor,
  surahAyahPathFor,
  surahLocalPagePathFor,
  surahPathFor,
  surahRouteContext,
} from "$lib/data/quran";

const ARABIC_ONLY_HELPERS = /\bsurahPath\b|\bsurahLocalPagePath\b|\bsurahAyahPath\b/;

const components = import.meta.glob("../../../**/*.svelte", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

describe("reader navigation regression guard", () => {
  it("the centralized route-aware helpers are exported from $lib/data/quran", () => {
    expect(typeof surahPathFor).toBe("function");
    expect(typeof surahLocalPagePathFor).toBe("function");
    expect(typeof surahAyahPathFor).toBe("function");
    expect(typeof globalPagePathFor).toBe("function");
    expect(typeof juzPathFor).toBe("function");
    expect(typeof surahRouteContext).toBe("function");
  });

  it("no .svelte under src/routes references the Arabic-only path helpers directly", () => {
    for (const [path, src] of Object.entries(components)) {
      expect(
        src,
        `Arabic-only helper used in ${path} -> use a *For(ctx, ...) helper instead`,
      ).not.toMatch(ARABIC_ONLY_HELPERS);
    }
  });

  it("the guard regex never matches the *For variants (false-positive check)", () => {
    expect(ARABIC_ONLY_HELPERS.test("surahPathFor")).toBe(false);
    expect(ARABIC_ONLY_HELPERS.test("surahLocalPagePathFor")).toBe(false);
    expect(ARABIC_ONLY_HELPERS.test("surahAyahPathFor")).toBe(false);
  });
});
