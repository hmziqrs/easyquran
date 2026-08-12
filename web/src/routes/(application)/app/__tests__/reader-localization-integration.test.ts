import { describe, expect, it } from "vite-plus/test";
import { surahAyahPathFor, surahLocalPagePathFor } from "$lib/data/quran";
import { readerHrefFor } from "$lib/i18n/reader";
import { deLocalizeUrl } from "$lib/paraglide/runtime";

const TRANSLATION = {
  kind: "translation",
  lang: "en",
  translator: "dr.mustafa.khattab",
} as const;

function switchUiLocale(href: string, locale: "en" | "ar"): string {
  const canonical = deLocalizeUrl(new URL(href, "https://easyquran.fyi"));
  return readerHrefFor(
    locale,
    `${canonical.pathname}${canonical.search}${canonical.hash}`,
  );
}

describe("reader UI locale navigation", () => {
  it("preserves source, dotted translator id, page, query, and ayah hash", () => {
    const ayahUrl = new URL(
      surahAyahPathFor(TRANSLATION, { num: 30, slug: "ar-rum" }, 7, 12),
      "https://easyquran.fyi",
    );
    ayahUrl.searchParams.set("view", "reading");
    const canonical = `${ayahUrl.pathname}${ayahUrl.search}${ayahUrl.hash}`;
    const arabicUi = readerHrefFor("ar", canonical);

    expect(arabicUi).toBe(
      "/ar/app/ar-rum/t/en/dr.mustafa.khattab/page/7?view=reading#ayah-30-12",
    );
    expect(switchUiLocale(arabicUi, "en")).toBe(
      "/en/app/ar-rum/t/en/dr.mustafa.khattab/page/7?view=reading#ayah-30-12",
    );
  });

  it("switches UI locale without dropping a translation on canonical page one", () => {
    const canonical = surahLocalPagePathFor(TRANSLATION, "ar-rum", 1);
    expect(switchUiLocale(readerHrefFor("en", canonical), "ar")).toBe(
      "/ar/app/ar-rum/t/en/dr.mustafa.khattab",
    );
  });
});
