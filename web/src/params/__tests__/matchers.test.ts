import { describe, expect, it, vi } from "vite-plus/test";

import { textVariantEntries } from "#lib/seo/render.js";
import { QURAN_DATA } from "#lib/server/quran-data.js";

vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
}));

import { matchMarketingText as marketingText, matchSurahSlug as surahSlug } from "../../params.js";

/**
 * Dispatcher split for the two same-depth single-segment `.md` families
 * (scheme A moved the reader tree to the site root, so `[slug].md` and
 * `[surah].md` would otherwise be resolved by SvelteKit's param-name
 * tie-break). The matchers make the two families disjoint by construction:
 * `about.md` can only be the marketing twin, `al-baqarah.md` only the reader
 * twin, and an unknown slug matches neither (404). Kit 3 matchers return the
 * parsed param on match and `undefined` on no-match (the boolean `true`/`false`
 * era is gone), so these specs assert the echoed slug or `undefined`.
 */
describe("[slug=marketingText] matcher", () => {
  it("admits exactly the six marketing text-variant slugs", () => {
    expect(marketingText("index")).toBe("index");
    expect(marketingText("about")).toBe("about");
    expect(marketingText("faq")).toBe("faq");
    expect(marketingText("contact")).toBe("contact");
    expect(marketingText("privacy")).toBe("privacy");
    expect(marketingText("terms")).toBe("terms");
  });

  it("rejects reader slugs, reserved words, and junk", () => {
    expect(marketingText("al-baqarah")).toBeUndefined();
    expect(marketingText("juz")).toBeUndefined();
    expect(marketingText("search")).toBeUndefined();
    expect(marketingText("Index")).toBeUndefined();
    expect(marketingText("")).toBeUndefined();
    expect(marketingText("about/extra")).toBeUndefined();
  });

  it("admits exactly the prerendered textVariantEntries set", () => {
    const entries = textVariantEntries().map((entry) => entry.slug);
    expect(entries).toHaveLength(6);
    for (const slug of entries) expect(marketingText(slug)).toBe(slug);
  });
});

describe("[surah=surahSlug] matcher", () => {
  it("admits baked surah slugs", () => {
    for (const surah of QURAN_DATA.surahs) {
      expect(surahSlug(surah.slug)).toBe(surah.slug);
    }
    expect(QURAN_DATA.surahs.length).toBe(114);
  });

  it("rejects every reserved and product top-level segment", () => {
    for (const word of [
      "juz",
      "page",
      "hizb",
      "rub",
      "t",
      "surah",
      "pages",
      "yours",
      "search",
      "settings",
      "bookmarks",
      "index",
      "about",
      "faq",
      "contact",
      "privacy",
      "terms",
    ]) {
      expect(surahSlug(word), word).toBeUndefined();
    }
  });

  it("rejects non-grammar input", () => {
    expect(surahSlug("Al-Fatihah")).toBeUndefined();
    expect(surahSlug("-al-fatihah")).toBeUndefined();
    expect(surahSlug("al-fatihah-")).toBeUndefined();
    expect(surahSlug("")).toBeUndefined();
    expect(surahSlug("2")).toBeUndefined();
  });

  it("keeps the two .md families disjoint (dispatcher invariant)", () => {
    const marketing = textVariantEntries().map((entry) => entry.slug);
    for (const surah of QURAN_DATA.surahs) {
      expect(marketing, surah.slug).not.toContain(surah.slug);
      expect(marketingText(surah.slug)).toBeUndefined();
    }
    for (const slug of marketing) expect(surahSlug(slug)).toBeUndefined();
  });
});
