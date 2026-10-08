import { describe, expect, it } from "vite-plus/test";

import { surahAyahPathFor, surahPathFor } from "#lib/data/quran.js";
import { readerHrefFor } from "#lib/i18n/reader.js";
import { deLocalizeUrl } from "#lib/paraglide/runtime.js";

const TRANSLATION = {
  kind: "translation",
  lang: "en",
  translator: "dr.mustafa.khattab",
} as const;

function switchUiLocale(href: string, locale: "en" | "ar"): string {
  const canonical = deLocalizeUrl(new URL(href, "https://easyquran.fyi"));
  return readerHrefFor(locale, `${canonical.pathname}${canonical.search}${canonical.hash}`);
}

describe("reader UI locale navigation", () => {
  it("preserves source, dotted translator id, query, and ayah hash", () => {
    const ayahUrl = new URL(
      surahAyahPathFor(TRANSLATION, { num: 30, slug: "ar-rum" }, 12),
      "https://easyquran.fyi",
    );
    ayahUrl.searchParams.set("view", "reading");
    const canonical = `${ayahUrl.pathname}${ayahUrl.search}${ayahUrl.hash}`;
    const arabicUi = readerHrefFor("ar", canonical);

    expect(arabicUi).toBe("/ar/ar-rum/t/en/dr.mustafa.khattab?view=reading#ayah-30-12");
    expect(switchUiLocale(arabicUi, "en")).toBe(
      "/ar-rum/t/en/dr.mustafa.khattab?view=reading#ayah-30-12",
    );
  });

  it("switches UI locale without dropping a translation on the surah root", () => {
    const canonical = surahPathFor(TRANSLATION, "ar-rum");
    expect(switchUiLocale(readerHrefFor("en", canonical), "ar")).toBe(
      "/ar/ar-rum/t/en/dr.mustafa.khattab",
    );
  });
});
