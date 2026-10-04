import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { mount } from "svelte";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { nav, readerStub, mountStub } = vi.hoisted(() => ({
  // SAFETY: empty params literal widens to the page.params Record<string,string> contract; the
  // Arabic row never reads a string entry the test leaves unset
  nav: { url: { hash: "" }, params: {} as Record<string, string> },
  readerStub: {
    isVerseMode: true,
    isReadingMode: false,
    // SAFETY: null seeds the nullable openNote union; no test assigns before the row reads it
    openNote: null as string | null,
  },
  mountStub: () => {},
}));

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: nav }));
vi.mock("$lib/stores/reader.svelte", () => ({ reader: readerStub }));
vi.mock("../VerseTools.svelte", () => ({ default: mountStub }));

import { toArabicDigits } from "$lib/data/quran";

import VerseRow from "../VerseRow.svelte";

function findWebRoot(): string {
  let dir = process.cwd();
  for (let i = 0; i < 12; i += 1) {
    if (existsSync(resolve(dir, "src/app.html")) && existsSync(resolve(dir, "package.json"))) {
      return dir;
    }
    dir = resolve(dir, "..");
  }
  throw new Error("ayah-ornament guard: could not locate web/ root from " + process.cwd());
}

const LAYOUT_CSS = readFileSync(resolve(findWebRoot(), "src/routes/layout.css"), "utf8");

let target: HTMLElement;

beforeEach(() => {
  readerStub.isVerseMode = true;
  readerStub.isReadingMode = false;
  readerStub.openNote = null;
  nav.params = {};
  target = document.createElement("div");
  document.body.appendChild(target);
});

describe("end-of-ayah ornament run (36afeb2 regression guard)", () => {
  it("closes every Arabic verse with U+06DD followed by toArabicDigits(n) on the ornament class", () => {
    // 286 forces multi-digit mapping (٢٨٦), not a hardcoded single glyph
    mount(VerseRow, { target, props: { text: "ٱللَّهُ", n: 286, vKey: "2:286" } });
    const ornament = target.querySelector(".verse-text .ayah-ornament");
    expect(ornament).not.toBeNull();
    expect(ornament?.getAttribute("data-verse-anchor")).toBe("2:286");
    expect(ornament?.textContent).toBe(`\u06DD${toArabicDigits(286)}`);
    expect(ornament?.textContent).toBe("\u06DD٢٨٦");
  });

  it("keeps a composing webfont ahead of Amiri in the .ayah-ornament fallback chain", () => {
    const utilityStart = LAYOUT_CSS.indexOf("@utility ayah-ornament {");
    expect(utilityStart).toBeGreaterThan(-1);
    const utility = LAYOUT_CSS.slice(utilityStart, LAYOUT_CSS.indexOf("}", utilityStart));
    const families = [...utility.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect(families[0]).toBe("Ayah Ornament");
    // Scheherazade New composes the ring; Amiri only carries both halves without tofu
    expect(families.indexOf("Scheherazade New")).toBeGreaterThan(-1);
    expect(families.indexOf("Scheherazade New")).toBeLessThan(families.indexOf("Amiri"));
    // The composing fallback must be loadable: a chain entry nothing loads still falls to Amiri
    expect(LAYOUT_CSS).toContain('@import "@fontsource/scheherazade-new/arabic-400.css";');
  });
});
