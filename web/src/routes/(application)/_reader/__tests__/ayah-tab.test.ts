import { describe, expect, it } from "vite-plus/test";

import { QURAN_DATA } from "#lib/server/quran-data.js";

import { ayahTabSurah } from "../ayah-tab";

describe("ayahTabSurah fallback (sidebar Ayah tab on every route)", () => {
  it("prefers the route's surah slug", () => {
    const current = ayahTabSurah(QURAN_DATA, { surah: "al-kahf" }, 36);
    expect(current.num).toBe(18);
    expect(current.slug).toBe("al-kahf");
  });

  it("falls back to the reader's current surah off a surah route", () => {
    expect(ayahTabSurah(QURAN_DATA, {}, 67).num).toBe(67);
    expect(ayahTabSurah(QURAN_DATA, { n: "42" }, 2).num).toBe(2);
  });

  it("falls back to surah 1 when the reader current is out of range", () => {
    expect(ayahTabSurah(QURAN_DATA, {}, 999).num).toBe(1);
  });

  it("an unknown slug degrades to the reader current, not an empty panel", () => {
    expect(ayahTabSurah(QURAN_DATA, { surah: "not-a-surah" }, 19).num).toBe(19);
  });
});
