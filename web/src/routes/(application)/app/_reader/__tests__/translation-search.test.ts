import { describe, expect, it } from "vite-plus/test";
import {
  translationMatchesQuery,
  translationTokenMatches,
} from "../translation-search";

const fields = {
  name: "Bayan-ul-Quran",
  translator: "Dr. Israr Ahmad",
  language: "Urdu",
  languageCode: "ur",
  country: "Pakistan",
  autonym: "اردو",
};

describe("translationTokenMatches", () => {
  it("matches substrings across every field", () => {
    expect(translationTokenMatches("bayan", fields)).toBe(true);
    expect(translationTokenMatches("israr", fields)).toBe(true);
    expect(translationTokenMatches("urdu", fields)).toBe(true);
    expect(translationTokenMatches("ur", fields)).toBe(true);
    expect(translationTokenMatches("pakistan", fields)).toBe(true);
    expect(translationTokenMatches("quran", fields)).toBe(true);
  });

  it("tolerates one edit in tokens of four or more characters", () => {
    expect(translationTokenMatches("englsh", { ...fields, language: "English" })).toBe(true);
    expect(translationTokenMatches("pakstan", fields)).toBe(true);
    expect(translationTokenMatches("urduu", fields)).toBe(true);
  });

  it("does not fuzzy-match short tokens", () => {
    expect(translationTokenMatches("urx", fields)).toBe(false);
  });

  it("rejects distant tokens", () => {
    expect(translationTokenMatches("engleesh", { ...fields, language: "English" })).toBe(false);
  });

  it("strips diacritics before comparing", () => {
    const garcia = { ...fields, translator: "Hernán García" };
    expect(translationTokenMatches("garcia", garcia)).toBe(true);
    expect(translationTokenMatches("hernan", garcia)).toBe(true);
    expect(translationTokenMatches("garcía", garcia)).toBe(true);
  });

  it("skips empty country fields instead of matching them", () => {
    expect(translationTokenMatches("kurdish", { ...fields, country: "" })).toBe(false);
  });

  it("matches the native-script autonym (stress S2: اردو finds Urdu)", () => {
    expect(translationTokenMatches("اردو", fields)).toBe(true);
    expect(translationMatchesQuery("اردو", fields)).toBe(true);
  });

  it("skips null autonyms instead of matching them", () => {
    expect(translationTokenMatches("اردو", { ...fields, autonym: null })).toBe(false);
  });

  it("matches the Chinese autonym without whitespace splitting", () => {
    const zh = { ...fields, language: "Chinese", languageCode: "zh", autonym: "中文" };
    expect(translationMatchesQuery("中文", zh)).toBe(true);
  });

  it("strips Arabic harakat from autonym tokens before comparing", () => {
    // normalize() is NFD + combining-mark strip: harakat are Mn, so a
    // vocalized query matches the bare autonym.
    const ar = { ...fields, language: "Arabic", languageCode: "ar", autonym: "العربية" };
    expect(translationTokenMatches("العَرَبِيَّة", ar)).toBe(true);
  });
});

describe("translationMatchesQuery", () => {
  it("requires every token to hit some field", () => {
    expect(translationMatchesQuery("israr urdu", fields)).toBe(true);
    expect(translationMatchesQuery("israr french", fields)).toBe(false);
  });

  it("keeps every-token AND semantics working across scripts (stress S2)", () => {
    // With the autonym in the haystack both tokens hit the SAME Urdu rows,
    // so the mixed-script query narrows correctly instead of returning zero.
    expect(translationMatchesQuery("urdu اردو", fields)).toBe(true);
    expect(translationMatchesQuery("اردو french", fields)).toBe(false);
  });

  it("matches everything on an empty query", () => {
    expect(translationMatchesQuery("", fields)).toBe(true);
    expect(translationMatchesQuery("   ", fields)).toBe(true);
  });
});
