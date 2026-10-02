import { textVariantEntries } from "$lib/seo/render";
import { QURAN_DATA } from "$lib/server/quran-data";
import { describe, expect, it, vi } from "vite-plus/test";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

import { match as marketingText } from "../marketingText";
import { match as surahSlug } from "../surahSlug";

/**
 * Dispatcher split for the two same-depth single-segment `.md` families
 * (scheme A moved the reader tree to the site root, so `[slug].md` and
 * `[surah].md` would otherwise be resolved by SvelteKit's param-name
 * tie-break). The matchers make the two families disjoint by construction:
 * `about.md` can only be the marketing twin, `al-baqarah.md` only the reader
 * twin, and an unknown slug matches neither (404).
 */
describe("[slug=marketingText] matcher", () => {
  it("admits exactly the six marketing text-variant slugs", () => {
    expect(marketingText("index")).toBe(true);
    expect(marketingText("about")).toBe(true);
    expect(marketingText("faq")).toBe(true);
    expect(marketingText("contact")).toBe(true);
    expect(marketingText("privacy")).toBe(true);
    expect(marketingText("terms")).toBe(true);
  });

  it("rejects reader slugs, reserved words, and junk", () => {
    expect(marketingText("al-baqarah")).toBe(false);
    expect(marketingText("juz")).toBe(false);
    expect(marketingText("search")).toBe(false);
    expect(marketingText("Index")).toBe(false);
    expect(marketingText("")).toBe(false);
    expect(marketingText("about/extra")).toBe(false);
  });

  it("admits exactly the prerendered textVariantEntries set", () => {
    const entries = textVariantEntries().map((entry) => entry.slug);
    expect(entries).toHaveLength(6);
    for (const slug of entries) expect(marketingText(slug)).toBe(true);
  });
});

describe("[surah=surahSlug] matcher", () => {
  it("admits baked surah slugs", () => {
    for (const surah of QURAN_DATA.surahs) {
      expect(surahSlug(surah.slug)).toBe(true);
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
      expect(surahSlug(word), word).toBe(false);
    }
  });

  it("rejects non-grammar input", () => {
    expect(surahSlug("Al-Fatihah")).toBe(false);
    expect(surahSlug("-al-fatihah")).toBe(false);
    expect(surahSlug("al-fatihah-")).toBe(false);
    expect(surahSlug("")).toBe(false);
    expect(surahSlug("2")).toBe(false);
  });

  it("keeps the two .md families disjoint (dispatcher invariant)", () => {
    const marketing = textVariantEntries().map((entry) => entry.slug);
    for (const surah of QURAN_DATA.surahs) {
      expect(marketing, surah.slug).not.toContain(surah.slug);
      expect(marketingText(surah.slug)).toBe(false);
    }
    for (const slug of marketing) expect(surahSlug(slug)).toBe(false);
  });
});
