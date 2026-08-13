import {
  marketingSeoLinks,
  readerCanonicalEntryPath,
  readerCanonicalPath,
  readerCanonicalUrl,
  readerEntryPath,
} from "$lib/i18n/seo";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

describe("localized SEO links", () => {
  it("emits reciprocal home publications and English x-default", () => {
    expect(marketingSeoLinks("home", "en")).toEqual({
      canonical: "https://easyquran.fyi/",
      alternates: [
        { hreflang: "en", href: "https://easyquran.fyi/" },
        { hreflang: "ar", href: "https://easyquran.fyi/ar/" },
        { hreflang: "x-default", href: "https://easyquran.fyi/" },
      ],
    });
    expect(marketingSeoLinks("home", "ar").canonical).toBe("https://easyquran.fyi/ar/");
  });

  it("does not invent an Arabic publication for an English-only page", () => {
    expect(marketingSeoLinks("about", "en")).toEqual({
      canonical: "https://easyquran.fyi/about",
      alternates: [
        { hreflang: "en", href: "https://easyquran.fyi/about" },
        { hreflang: "x-default", href: "https://easyquran.fyi/about" },
      ],
    });
    expect(() => marketingSeoLinks("about", "ar")).toThrow(/unpublished marketing page/);
  });

  it("canonicalizes every reader UI variant to en without changing translation source", () => {
    const translated = "/app/ar-rum/t/ms/basmeih/page/7?view=compact#ayah-30-12";
    expect(readerCanonicalPath(translated)).toBe(
      "/en/app/ar-rum/t/ms/basmeih/page/7?view=compact#ayah-30-12",
    );
    expect(readerCanonicalUrl(translated)).toBe(
      "https://easyquran.fyi/en/app/ar-rum/t/ms/basmeih/page/7?view=compact#ayah-30-12",
    );
  });

  it("never creates reader markdown or text variants", () => {
    const canonical = readerCanonicalPath("/app/al-fatihah");
    expect(canonical).toBe("/en/app/al-fatihah");
    expect(canonical).not.toMatch(/\.(?:md|txt)$/);
  });

  it("canonicalizes both bounded reader entry routes without inventing source context", () => {
    expect(readerEntryPath("ar", "home")).toBe("/ar/app");
    expect(readerEntryPath("ar", "juz-index")).toBe("/ar/app/juz");
    expect(readerCanonicalEntryPath("home")).toBe("/en/app");
    expect(readerCanonicalEntryPath("juz-index")).toBe("/en/app/juz");
  });
});
