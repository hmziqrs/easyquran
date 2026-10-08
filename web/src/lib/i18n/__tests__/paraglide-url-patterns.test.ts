import { describe, expect, it } from "vite-plus/test";

import { deLocalizeUrl, localizeHref } from "#lib/paraglide/runtime.js";

/**
 * Scheme A round-trip fixtures for the paraglide default `/:locale/...` URL
 * pattern. paraglide.config.js deliberately configures no custom urlPatterns,
 * so the generated runtime must behave as the default-pattern implementations
 * do: `en` (base locale) is unprefixed, every other locale gets `/{locale}`,
 * and de-localization strips exactly one leading locale segment. If the
 * runtime is ever regenerated with custom patterns again
 * (TREE_SHAKE_DEFAULT_URL_PATTERN_USED flips back to false), these pin the
 * regression loudly instead of silently re-prefixing the site.
 */
describe("paraglide default URL pattern round-trip (scheme A)", () => {
  const FIXTURES = ["/", "/al-baqarah", "/t/en/sahih/page/7", "/search", "/juz/1"] as const;

  it("localizes ar by prefixing /ar and leaves en unprefixed", () => {
    for (const path of FIXTURES) {
      const expectedAr = path === "/" ? "/ar/" : `/ar${path}`;
      expect(localizeHref(path, { locale: "ar" })).toBe(expectedAr);
      // en is the base locale: identity on a canonical (unprefixed) path.
      expect(localizeHref(path, { locale: "en" })).toBe(path);
    }
  });

  it("de-localizes the /ar spelling back to the canonical path", () => {
    for (const path of FIXTURES) {
      expect(deLocalizeUrl(path === "/" ? "/ar/" : `/ar${path}`).pathname).toBe(path);
      // Unprefixed paths are already canonical: identity.
      expect(deLocalizeUrl(path).pathname).toBe(path);
    }
  });

  it("round-trips a localized ar href back to its en canonical byte-for-byte", () => {
    for (const path of FIXTURES) {
      const ar = localizeHref(path, { locale: "ar" });
      expect(deLocalizeUrl(ar).pathname).toBe(path);
    }
  });

  it("keeps query and fragment attached through localization", () => {
    const href = "/al-baqarah?view=reading#ayah-2-255";
    const ar = localizeHref(href, { locale: "ar" });
    expect(ar).toBe(`/ar${href}`);
    expect(deLocalizeUrl(ar).pathname + deLocalizeUrl(ar).search + deLocalizeUrl(ar).hash).toBe(
      href,
    );
  });

  it("does not treat reader segments as locale segments", () => {
    // No baked slug, range segment, or reserved word may equal a locale code —
    // otherwise /ar/... and /en/... would be ambiguous with content paths.
    const readerWords = [
      "al-baqarah",
      "page",
      "pages",
      "juz",
      "hizb",
      "rub",
      "t",
      "surah",
      "search",
      "settings",
      "bookmarks",
      "yours",
    ];
    for (const word of readerWords) {
      expect(deLocalizeUrl(`/${word}`).pathname).toBe(`/${word}`);
    }
  });
});
