import { readFileSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";
import {
  marketingFooterLinks,
  marketingHomeHref,
  marketingLocaleFromPath,
  marketingLocaleLinks,
  marketingReaderHomeHref,
  resolveMarketingCopy,
} from "$lib/i18n/marketing-copy";

function readJson(path: string): Record<string, string> {
  return JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8")) as Record<string, string>;
}

function parameters(value: string): string[] {
  return [...value.matchAll(/\{([a-z]+)\}/gi)].map((match) => match[1]!).sort();
}

describe("marketing message catalogs", () => {
  const english = readJson("../../../../messages/en.json");
  const arabic = readJson("../../../../messages/ar.json");

  it("keeps exact key and parameter parity", () => {
    expect(Object.keys(arabic).sort()).toEqual(Object.keys(english).sort());

    for (const key of Object.keys(english)) {
      expect(parameters(arabic[key]!)).toEqual(parameters(english[key]!));
    }
  });

  it("contains non-empty plain text only", () => {
    for (const catalog of [english, arabic]) {
      for (const value of Object.values(catalog)) {
        expect(value.trim()).not.toBe("");
        expect(value).not.toMatch(/<\/?[a-z][^>]*>/i);
      }
    }
  });
});

describe("marketing copy resolver", () => {
  it("resolves each locale explicitly without leaking prior calls", () => {
    const english = resolveMarketingCopy("en");
    const arabic = resolveMarketingCopy("ar");
    const englishAgain = resolveMarketingCopy("en");

    expect(english.direction).toBe("ltr");
    expect(arabic.direction).toBe("rtl");
    expect(arabic.landing.heroTitle).not.toBe(english.landing.heroTitle);
    expect(englishAgain.landing.heroTitle).toBe(english.landing.heroTitle);
  });

  it("keeps stable structural IDs across locales", () => {
    const english = resolveMarketingCopy("en");
    const arabic = resolveMarketingCopy("ar");

    expect(arabic.landing.values.map((item) => item.id)).toEqual(
      english.landing.values.map((item) => item.id),
    );
    expect(arabic.landing.roadmap.map((item) => item.id)).toEqual(
      english.landing.roadmap.map((item) => item.id),
    );
  });

  it("uses typed parameterized messages for control labels", () => {
    const english = resolveMarketingCopy("en").tweaks;
    const arabic = resolveMarketingCopy("ar").tweaks;

    expect(english.colourInputLabel("Background")).toBe("Background colour");
    expect(arabic.colourInputLabel("الخلفية")).toBe("لون الخلفية");
    expect(arabic.toggleStatusLabel("التحليلات", "مفعّل")).toContain("التحليلات");
  });
});

describe("marketing locale links", () => {
  it("recognizes only the Arabic marketing prefix", () => {
    expect(marketingLocaleFromPath("/")).toBe("en");
    expect(marketingLocaleFromPath("/about")).toBe("en");
    expect(marketingLocaleFromPath("/ar/")).toBe("ar");
    expect(marketingLocaleFromPath("/arbitrary")).toBe("en");
  });

  it("uses canonical marketing and reader helpers", () => {
    expect(marketingHomeHref("en")).toBe("/");
    expect(marketingHomeHref("ar")).toBe("/ar/");
    expect(marketingReaderHomeHref("en")).toBe("/en/app");
    expect(marketingReaderHomeHref("ar")).toBe("/ar/app");
  });

  it("does not expose unpublished Arabic marketing links", () => {
    const links = marketingFooterLinks("ar");

    expect(links.company).toEqual([]);
    expect(links.legal).toEqual([]);
    expect(links.product.every((link) => link.href.startsWith("/ar/"))).toBe(true);
  });

  it("marks one locale switch link current", () => {
    const links = marketingLocaleLinks("ar");

    expect(links.filter((link) => link.current)).toHaveLength(1);
    expect(links.find((link) => link.current)?.locale).toBe("ar");
    expect(links.every((link) => link.label.length > 0)).toBe(true);
  });
});
