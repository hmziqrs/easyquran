import type { UiLocale } from "$lib/i18n/locales";
import {
  MARKETING_PATHS,
  MARKETING_PUBLICATIONS,
  isMarketingPageId,
  isMarketingPublished,
  marketingHref,
  publishedMarketingLocales,
  type MarketingPageId,
  type MarketingPath,
} from "$lib/i18n/marketing";
import { describe, expect, expectTypeOf, it } from "vite-plus/test";

const PAGE_IDS = ["home", "about", "faq", "contact", "privacy", "terms"] as const;

describe("marketing publication matrix", () => {
  it("owns every current canonical marketing page", () => {
    expect(MARKETING_PATHS).toEqual({
      home: "/",
      about: "/about",
      faq: "/faq",
      contact: "/contact",
      privacy: "/privacy",
      terms: "/terms",
    });
    expect(Object.keys(MARKETING_PUBLICATIONS)).toEqual(PAGE_IDS);
    expectTypeOf<MarketingPageId>().toEqualTypeOf<(typeof PAGE_IDS)[number]>();
    expectTypeOf<MarketingPath>().toEqualTypeOf<
      "/" | "/about" | "/faq" | "/contact" | "/privacy" | "/terms"
    >();
  });

  it("publishes every page in English and only home in Arabic", () => {
    for (const pageId of PAGE_IDS) {
      expect(isMarketingPublished(pageId, "en")).toBe(true);
      expect(publishedMarketingLocales(pageId)).toContain("en");
      expect(Object.isFrozen(publishedMarketingLocales(pageId))).toBe(true);
    }

    expect(publishedMarketingLocales("home")).toEqual(["en", "ar"]);
    for (const pageId of PAGE_IDS.filter((id) => id !== "home")) {
      expect(publishedMarketingLocales(pageId)).toEqual(["en"]);
      expect(isMarketingPublished(pageId, "ar")).toBe(false);
    }
  });

  it("recognizes page IDs without accepting paths or inherited keys", () => {
    for (const pageId of PAGE_IDS) expect(isMarketingPageId(pageId)).toBe(true);
    for (const value of ["app", "/", "/about", "toString", "__proto__", "", null]) {
      expect(isMarketingPageId(value)).toBe(false);
    }
  });
});

describe("marketingHref", () => {
  it("keeps English marketing canonical paths unprefixed", () => {
    for (const pageId of PAGE_IDS) {
      expect(marketingHref(pageId, "en")).toBe(MARKETING_PATHS[pageId]);
    }
  });

  it("emits only published Arabic routes", () => {
    expect(marketingHref("home", "ar")).toBe("/ar/");
    for (const pageId of PAGE_IDS.filter((id) => id !== "home")) {
      expect(marketingHref(pageId, "ar")).toBeNull();
    }
  });

  it("never falls back for unknown pages or locales", () => {
    // SAFETY: "app" is deliberately not a MarketingPageId — the cast only lets the invalid value past the compiler so the null-return path is exercised.
    expect(marketingHref("app" as MarketingPageId, "en")).toBeNull();
    // SAFETY: "de" is deliberately not a UiLocale — the cast feeds the unsupported-locale throw path.
    expect(() => marketingHref("home", "de" as UiLocale)).toThrowError(
      new TypeError("Unsupported UI locale: de"),
    );
  });
});
