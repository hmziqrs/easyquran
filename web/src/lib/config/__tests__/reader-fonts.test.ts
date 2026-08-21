import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  ARABIC_FONTS,
  ARABIC_FONT_IDS,
  TRANSLATION_FAMILIES,
} from "$lib/config/reader-fonts";
import {
  ARABIC_FONT_MAX,
  ARABIC_FONT_MIN,
  TRANSLATION_FONT_MAX,
  TRANSLATION_FONT_MIN,
} from "$lib/stores/reader-core.svelte";

function findWebRoot(): string {
  let dir = process.cwd();
  for (let i = 0; i < 12; i += 1) {
    if (existsSync(resolve(dir, "src/app.html")) && existsSync(resolve(dir, "package.json"))) {
      return dir;
    }
    dir = resolve(dir, "..");
  }
  throw new Error("reader-fonts literal-sync guard: could not locate web/ root from " + process.cwd());
}

const APP_HTML = readFileSync(resolve(findWebRoot(), "src/app.html"), "utf8");

describe("app.html pre-paint literal sync", () => {
  it("mirrors every ARABIC_FONT_IDS entry in the arabicFont allowlist", () => {
    for (const id of ARABIC_FONT_IDS) {
      expect(
        APP_HTML.includes(`af === "${id}"`),
        `app.html arabicFont allowlist is missing "${id}"`,
      ).toBe(true);
    }
  });

  it("mirrors the translationSize bounds from the TS constants", () => {
    expect(APP_HTML.includes(`r.translationSize >= ${TRANSLATION_FONT_MIN}`)).toBe(true);
    expect(APP_HTML.includes(`r.translationSize <= ${TRANSLATION_FONT_MAX}`)).toBe(true);
  });

  it("mirrors the existing fontSize bounds from the TS constants", () => {
    expect(APP_HTML.includes(`r.fontSize >= ${ARABIC_FONT_MIN}`)).toBe(true);
    expect(APP_HTML.includes(`r.fontSize <= ${ARABIC_FONT_MAX}`)).toBe(true);
  });

  it("rejects ids and bounds the allowlist must not carry (guard is not toothless)", () => {
    expect(APP_HTML.includes('af === "kufi"')).toBe(false);
    expect(APP_HTML.includes("r.translationSize >= 12")).toBe(false);
    expect(APP_HTML.includes("r.fontSize >= 20")).toBe(false);
  });
});

describe("reader font registry", () => {
  it("defaults stay byte-free: only non-default fonts lazy-load files", () => {
    const amiri = ARABIC_FONTS.find((f) => f.id === "amiri");
    expect(amiri?.file).toBeUndefined();
    for (const id of ["scheherazade-new", "noto-naskh-arabic"]) {
      const font = ARABIC_FONTS.find((f) => f.id === id);
      expect(font?.file).toBeTypeOf("function");
    }
  });

  it("stacks lead with their own family and ids match the allowlist", () => {
    expect(ARABIC_FONTS.map((f) => f.id)).toEqual([...ARABIC_FONT_IDS]);
    for (const font of ARABIC_FONTS) {
      expect(font.stack.startsWith(`"${font.family}"`)).toBe(true);
    }
  });

  it("translation families are system stacks only", () => {
    expect(TRANSLATION_FAMILIES.map((f) => f.id)).toEqual(["sans", "serif"]);
    for (const family of TRANSLATION_FAMILIES) {
      expect(family.stack.includes("fontsource")).toBe(false);
    }
  });
});
