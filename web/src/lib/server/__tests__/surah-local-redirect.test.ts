import { QURAN_DATA } from "$lib/server/quran-data";
import { surahLocalRedirectTarget } from "$lib/server/reader-route";
import { describe, expect, it } from "vite-plus/test";

/**
 * Exhaustive over the baked surah-local page map: every removed /page/N shape
 * redirects to the surah root anchored at the spread's first ayah.
 */
describe("surah-local page redirect targets (exhaustive over the baked map)", () => {
  it("anchors every surah x local page 2..count at its startAyah", () => {
    for (const surah of QURAN_DATA.surahs) {
      const pages = QURAN_DATA.surahLocalPages(surah.num);
      for (let localPage = 2; localPage <= pages.length; localPage += 1) {
        const spread = QURAN_DATA.surahLocalPage(surah.num, localPage);
        if (!spread) throw new Error(`missing ${surah.slug} page ${localPage}`);
        const target = surahLocalRedirectTarget(`/${surah.slug}/page/${localPage}`);
        expect(target).toEqual({
          path: `/${surah.slug}`,
          fragment: `#ayah-${surah.num}-${spread.startAyah}`,
        });
      }
    }
  });

  it("collapses local page 1 of every surah to the bare root", () => {
    for (const surah of QURAN_DATA.surahs) {
      expect(surahLocalRedirectTarget(`/${surah.slug}/page/1`)).toEqual({
        path: `/${surah.slug}`,
        fragment: "",
      });
    }
  });

  it("yields null for count+1 and non-page paths", () => {
    for (const surah of QURAN_DATA.surahs) {
      const count = QURAN_DATA.surahLocalPageCount(surah.num);
      expect(surahLocalRedirectTarget(`/${surah.slug}/page/${count + 1}`)).toBeNull();
    }
    expect(surahLocalRedirectTarget("/al-baqarah")).toBeNull();
    expect(surahLocalRedirectTarget("/page/2")).toBeNull();
  });

  it("round-trips the translated shape's lang/translator", () => {
    const spread = QURAN_DATA.surahLocalPage(30, 2);
    if (!spread) throw new Error("missing ar-rum page 2");
    const target = surahLocalRedirectTarget("/ar-rum/t/ms/basmeih/page/2");
    expect(target).toEqual({
      path: "/ar-rum/t/ms/basmeih",
      fragment: `#ayah-30-${spread.startAyah}`,
    });
    expect(surahLocalRedirectTarget("/ar-rum/t/ms/basmeih/page/1")).toEqual({
      path: "/ar-rum/t/ms/basmeih",
      fragment: "",
    });
    expect(surahLocalRedirectTarget("/ar-rum/t/ms/not-a-catalogue-entry/page/2")).toBeNull();
  });
});
