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
});

describe("translationMatchesQuery", () => {
  it("requires every token to hit some field", () => {
    expect(translationMatchesQuery("israr urdu", fields)).toBe(true);
    expect(translationMatchesQuery("israr french", fields)).toBe(false);
  });

  it("matches everything on an empty query", () => {
    expect(translationMatchesQuery("", fields)).toBe(true);
    expect(translationMatchesQuery("   ", fields)).toBe(true);
  });
});
