import type { CatalogEntry } from "$lib/data/quran-types";
import { QURAN_DATA } from "$lib/server/quran-data";
import { surahRouteNav } from "$lib/server/quran-page-shape";
import { describe, expect, it } from "vite-plus/test";

function surahAt(num: number): CatalogEntry {
  const entry = QURAN_DATA.surahByNum(num);
  if (!entry) throw new Error(`missing surah ${num}`);
  return entry;
}

describe("surahRouteNav end-card navigation", () => {
  it("mushaf start has no previous surah; next points at the Surah 2 root", () => {
    const nav = surahRouteNav(surahAt(1), 1);
    expect(nav.previousSurah).toBeNull();
    expect(nav.nextSurah).toMatchObject({ num: 2, slug: "al-baqarah" });
    expect(nav.nextSurah?.meaning.length ?? 0).toBeGreaterThan(0);
  });

  it("mushaf end has a previous surah and no next", () => {
    const nav = surahRouteNav(surahAt(114), 604);
    expect(nav.previousSurah).toMatchObject({ num: 113, slug: "al-falaq" });
    expect(nav.nextSurah).toBeNull();
  });

  it("mid-mushah surah links both neighbours with meanings for the end-card subtitle", () => {
    const nav = surahRouteNav(surahAt(2), 2);
    expect(nav.previousSurah).toMatchObject({ num: 1, slug: "al-fatihah" });
    expect(nav.nextSurah).toMatchObject({ num: 3, slug: "aal-i-imran" });
    expect(nav.previousSurah?.meaning.length ?? 0).toBeGreaterThan(0);
    expect(nav.nextSurah?.meaning.length ?? 0).toBeGreaterThan(0);
  });

  it("degraded manual jumps address the adjacent GLOBAL mushaf pages", () => {
    // Al-Baqarah page 1 IS global page 2 -> previous = page 1, next = page 3.
    const page = QURAN_DATA.surahLocalPage(2, 1);
    if (!page) throw new Error("missing al-baqarah page 1");
    const nav = surahRouteNav(surahAt(2), page.globalPage);
    expect(nav.previousPage).toEqual({ globalPage: page.globalPage - 1 });
    expect(nav.nextPage).toEqual({ globalPage: page.globalPage + 1 });
  });

  it("page 1 of the mushaf has no previous page and page 604 no next", () => {
    expect(surahRouteNav(surahAt(1), 1).previousPage).toBeNull();
    expect(surahRouteNav(surahAt(114), 604).nextPage).toBeNull();
  });
});
