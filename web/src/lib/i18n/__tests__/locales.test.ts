import { describe, expect, expectTypeOf, it } from "vite-plus/test";
import {
  DEFAULT_UI_LOCALE,
  SUPPORTED_UI_LOCALES,
  UI_LOCALES,
  assertUiLocale,
  isUiLocale,
  uiDirection,
  uiLocaleMetadata,
  type UiDirection,
  type UiLocale,
} from "$lib/i18n/locales";

describe("UI locale registry", () => {
  it("contains only reviewed English and Arabic metadata", () => {
    expect(DEFAULT_UI_LOCALE).toBe("en");
    expect(SUPPORTED_UI_LOCALES).toEqual(["en", "ar"]);
    expect(UI_LOCALES).toEqual({
      en: {
        locale: "en",
        endonym: "English",
        direction: "ltr",
        openGraphLocale: "en_US",
      },
      ar: {
        locale: "ar",
        endonym: "العربية",
        direction: "rtl",
        openGraphLocale: "ar_SA",
      },
    });
    expect(Object.isFrozen(UI_LOCALES)).toBe(true);
    expect(Object.values(UI_LOCALES).every((value) => Object.isFrozen(value))).toBe(true);
    expect(Object.isFrozen(SUPPORTED_UI_LOCALES)).toBe(true);
  });

  it("narrows supported locale values without fallback", () => {
    for (const value of ["en", "ar"]) expect(isUiLocale(value)).toBe(true);
    for (const value of ["EN", "ar-SA", "de", "", null, undefined, 1, {}]) {
      expect(isUiLocale(value)).toBe(false);
    }
  });

  it("throws for unsupported locale assertions", () => {
    expect(() => assertUiLocale("de")).toThrowError(new TypeError("Unsupported UI locale: de"));
  });

  it("resolves metadata and direction with strict types", () => {
    expect(uiLocaleMetadata("en")).toBe(UI_LOCALES.en);
    expect(uiLocaleMetadata("ar")).toBe(UI_LOCALES.ar);
    expect(uiDirection("en")).toBe("ltr");
    expect(uiDirection("ar")).toBe("rtl");
    expectTypeOf<UiLocale>().toEqualTypeOf<"en" | "ar">();
    expectTypeOf<UiDirection>().toEqualTypeOf<"ltr" | "rtl">();
  });
});
