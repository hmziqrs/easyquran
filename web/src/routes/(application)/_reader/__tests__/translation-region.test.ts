import { describe, expect, it } from "vite-plus/test";

import { TRANSLATIONS } from "#lib/data/translations.js";

import { REGION_LANGUAGES } from "../region-languages";
import { COUNTRY_TIMEZONES } from "../region-timezones";
import {
  baseLanguageCodes,
  countryFromTimeZone,
  languagePriority,
  regionsFromLanguageTags,
} from "../translation-region";

describe("countryFromTimeZone", () => {
  it("maps both the IANA and the legacy ICU spelling browsers report", () => {
    expect(countryFromTimeZone("Asia/Karachi")).toBe("PK");
    // Chrome reports Asia/Calcutta, Firefox Asia/Kolkata
    expect(countryFromTimeZone("Asia/Kolkata")).toBe("IN");
    expect(countryFromTimeZone("Asia/Calcutta")).toBe("IN");
    expect(countryFromTimeZone("Europe/Kyiv")).toBe("UA");
    expect(countryFromTimeZone("Europe/Kiev")).toBe("UA");
  });

  it("returns null for zones outside the ranked countries and for no zone", () => {
    // English-only countries are left out: English is pinned for everyone already.
    expect(countryFromTimeZone("America/New_York")).toBeNull();
    expect(countryFromTimeZone("Etc/UTC")).toBeNull();
    expect(countryFromTimeZone(undefined)).toBeNull();
    expect(countryFromTimeZone("")).toBeNull();
  });
});

describe("language tags", () => {
  it("reads country regions, skipping numeric regions and malformed tags", () => {
    expect(regionsFromLanguageTags(["ur-PK", "en", "es-419", "zh-Hant-TW", "ur-PK", "!!"])).toEqual(
      ["PK", "TW"],
    );
  });

  it("reduces tags to deduped base codes in order", () => {
    expect(baseLanguageCodes(["ur-PK", "EN-gb", "ur", ""])).toEqual(["ur", "en"]);
  });
});

describe("languagePriority", () => {
  it("is English then Arabic with no reader signals", () => {
    expect(languagePriority([], [])).toEqual(["en", "ar"]);
  });

  it("puts the preferred language first, English second, then browser, country, Arabic", () => {
    expect(languagePriority(["fr-FR", "de"], ["PK"])).toEqual([
      "fr",
      "en",
      "de",
      "ur",
      "pa",
      "sd",
      "ps",
      "ar",
    ]);
  });

  it("never repeats a code", () => {
    expect(languagePriority(["ur-PK", "en"], ["PK", "IN"]).filter((c) => c === "ur")).toHaveLength(
      1,
    );
  });
});

describe("region data", () => {
  const catalogueCodes = new Set(TRANSLATIONS.map((t) => t.languageCode));

  it("only names languages the catalogue carries", () => {
    for (const [country, codes] of Object.entries(REGION_LANGUAGES)) {
      for (const code of codes) expect(catalogueCodes.has(code), `${country}: ${code}`).toBe(true);
    }
  });

  it("gives every ranked country at least one timezone, and no other country", () => {
    expect(Object.keys(COUNTRY_TIMEZONES).sort()).toEqual(Object.keys(REGION_LANGUAGES).sort());
  });
});
