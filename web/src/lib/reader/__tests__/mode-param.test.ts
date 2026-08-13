import {
  READER_MODE_PARAM,
  modeParamMatches,
  parseModeParam,
  withModeParam,
} from "$lib/reader/mode-param";
import { describe, expect, it } from "vite-plus/test";

describe("parseModeParam", () => {
  it("returns the mode when the param is a valid reader mode", () => {
    expect(parseModeParam(new URL("https://h/app/al-fatihah?mode=reading"))).toBe("reading");
    expect(parseModeParam(new URL("https://h/app/al-fatihah?mode=verse"))).toBe("verse");
  });

  it("returns null when the param is absent", () => {
    expect(parseModeParam(new URL("https://h/app/al-fatihah"))).toBeNull();
  });

  it("returns null for an unknown or empty value", () => {
    expect(parseModeParam(new URL("https://h/app/al-fatihah?mode=foo"))).toBeNull();
    expect(parseModeParam(new URL("https://h/app/al-fatihah?mode="))).toBeNull();
  });
});

describe("withModeParam", () => {
  it("sets the mode param while preserving path, hash and other params", () => {
    const out = withModeParam(new URL("https://h/app/al-fatihah?page=2#ayah-1-1"), "reading");
    expect(out.searchParams.get(READER_MODE_PARAM)).toBe("reading");
    expect(out.searchParams.get("page")).toBe("2");
    expect(out.pathname).toBe("/app/al-fatihah");
    expect(out.hash).toBe("#ayah-1-1");
  });

  it("overwrites a stale mode param", () => {
    const out = withModeParam(new URL("https://h/app/al-fatihah?mode=reading"), "verse");
    expect(out.searchParams.get(READER_MODE_PARAM)).toBe("verse");
  });

  it("resolves a relative path string against the supplied base", () => {
    const out = withModeParam("/app/al-fatihah", "reading", "https://h");
    expect(out.href).toBe("https://h/app/al-fatihah?mode=reading");
  });
});

describe("modeParamMatches", () => {
  it("is true only when the param equals the given mode", () => {
    expect(modeParamMatches(new URL("https://h/app?mode=reading"), "reading")).toBe(true);
    expect(modeParamMatches(new URL("https://h/app?mode=verse"), "reading")).toBe(false);
    expect(modeParamMatches(new URL("https://h/app"), "verse")).toBe(false);
  });
});
