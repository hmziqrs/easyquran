import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import { textVariantSource } from "#lib/seo/render.js";
import { isLocalizedTextVariant } from "#lib/seo/text-variant-route.js";

describe("isLocalizedTextVariant (F18 reroute of /ar text variants)", () => {
  it("reroutes an ar-prefixed marketing .md or .txt onto its unprefixed twin", () => {
    expect(isLocalizedTextVariant("/ar/about.md", "/about.md")).toBe(true);
    expect(isLocalizedTextVariant("/ar/privacy.txt", "/privacy.txt")).toBe(true);
    expect(isLocalizedTextVariant("/ar/index.md", "/index.md")).toBe(true);
  });

  it("leaves unprefixed paths and mismatched candidates alone", () => {
    expect(isLocalizedTextVariant("/about.md", "/about.md")).toBe(false);
    expect(isLocalizedTextVariant("/ar/about.md", "/faq.md")).toBe(false);
    expect(isLocalizedTextVariant("/ar/about", "/about")).toBe(false);
  });

  it("rejects shapes that are not single-segment text variants", () => {
    expect(isLocalizedTextVariant("/ar/Bad.md", "/Bad.md")).toBe(false);
    expect(isLocalizedTextVariant("/ar/a/b.md", "/a/b.md")).toBe(false);
    expect(isLocalizedTextVariant("/ar/about.html", "/about.html")).toBe(false);
  });
});

describe("textVariantSource (F18 Arabic text variants)", () => {
  it("serves the Arabic twin for a page published in ar", () => {
    expect(textVariantSource("index", "ar")).toEqual({ path: "/ar", lang: "ar" });
  });

  it("serves the en canonical for en-only pages even under ar", () => {
    expect(textVariantSource("about", "ar")).toEqual({ path: "/about", lang: "en" });
    expect(textVariantSource("terms", "ar")).toEqual({ path: "/terms", lang: "en" });
  });

  it("serves the en canonical for the en locale", () => {
    expect(textVariantSource("index", "en")).toEqual({ path: "/", lang: "en" });
    expect(textVariantSource("faq", "en")).toEqual({ path: "/faq", lang: "en" });
  });

  it("falls back to the en canonical for an unknown slug", () => {
    expect(textVariantSource("missing", "ar")).toEqual({ path: "/missing", lang: "en" });
  });
});
