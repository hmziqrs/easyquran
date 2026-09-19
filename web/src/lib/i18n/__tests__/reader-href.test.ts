import {
  translationJuzPath,
  translationSegmentsFromId,
  translationSurahPath,
} from "$lib/data/quran";
import { TRANSLATIONS } from "$lib/data/translations";
import { readerHrefFor } from "$lib/i18n/reader";
import { describe, expect, it } from "vite-plus/test";

describe("canonical reader hrefs accept every baked translation id", () => {
  it("produces a canonical localized href for every id in the baked catalogue", () => {
    expect(TRANSLATIONS.length).toBeGreaterThan(0);
    for (const translation of TRANSLATIONS) {
      const { lang, translator } = translationSegmentsFromId(translation.id);
      expect(readerHrefFor("en", translationSurahPath("al-fatihah", lang, translator))).toBe(
        `/en/app/al-fatihah/t/${lang}/${translator}`,
      );
    }
  });

  it("accepts underscore translator segments from every provenance family", () => {
    // quranenc ids carry underscores (en.hilali_khan) — the historical crash
    // was exactly this family failing TRANSLATOR_SEGMENT.
    expect(readerHrefFor("en", "/app/al-fatihah/t/quranenc/en.hilali_khan")).toBe(
      "/en/app/al-fatihah/t/quranenc/en.hilali_khan",
    );
    // qul ids carry digits in the language segment (r158).
    const qul = TRANSLATIONS.find((translation) => translation.id.startsWith("qul."));
    expect(qul).toBeDefined();
    if (qul) {
      const { lang, translator } = translationSegmentsFromId(qul.id);
      expect(readerHrefFor("ar", translationSurahPath("al-fatihah", lang, translator))).toBe(
        `/ar/app/al-fatihah/t/${lang}/${translator}`,
      );
    }
    // tanzil dotted translators (en.sahih.int) keep working.
    expect(readerHrefFor("en", "/app/al-fatihah/t/en/sahih.int")).toBe(
      "/en/app/al-fatihah/t/en/sahih.int",
    );
  });

  it("accepts underscore translators on range and paged reader routes", () => {
    expect(readerHrefFor("en", "/app/t/quranenc/en.hilali_khan/juz/30")).toBe(
      "/en/app/t/quranenc/en.hilali_khan/juz/30",
    );
    expect(readerHrefFor("en", "/app/al-baqarah/t/quranenc/en.hilali_khan/page/2")).toBe(
      "/en/app/al-baqarah/t/quranenc/en.hilali_khan/page/2",
    );
  });

  it("round-trips translationJuzPath through readerHrefFor for a qul id", () => {
    const { lang, translator } = translationSegmentsFromId("qul.r158.bayan-ul-quran");
    expect(readerHrefFor("ar", translationJuzPath(lang, translator, 1))).toBe(
      `/ar/app/t/${lang}/${translator}/juz/1`,
    );
  });
});
