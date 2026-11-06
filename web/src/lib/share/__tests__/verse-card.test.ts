import { describe, expect, it } from "vite-plus/test";

import { CARD_WIDTH, isArabicScript, layoutCard, wrapWords, type CardInput } from "../verse-card";

const AYAH = "ٱللَّهُ لَآ إِلَٰهَ إِلَّا هُوَ ٱلْحَىُّ ٱلْقَيُّومُ";
const measure = (_font: string, text: string) => text.length * 10;

function input(overrides: Partial<CardInput> = {}): CardInput {
  return {
    verses: [{ text: AYAH, ref: "Al-Baqarah 2:255" }],
    reference: true,
    link: "https://easyquran.test/al-baqarah?ayah=2:255",
    brand: "EasyQuran",
    footer: "Read on EasyQuran",
    ...overrides,
  };
}

describe("isArabicScript", () => {
  it("detects Arabic-block text and rejects Latin", () => {
    expect(isArabicScript(AYAH)).toBe(true);
    expect(isArabicScript("Surah Al-Baqarah 2:255")).toBe(false);
    expect(isArabicScript("")).toBe(false);
  });
});

describe("wrapWords", () => {
  it("keeps every word in order, so Arabic text stays verbatim after wrapping", () => {
    const words = AYAH.split(/\s+/u);
    const lines = wrapWords(words, 120, (line) => line.length * 10);
    expect(lines.length).toBeGreaterThan(1);
    expect(lines.join(" ")).toBe(AYAH);
  });

  it("returns one line when everything fits", () => {
    expect(wrapWords(["a", "b"], 1000, (line) => line.length)).toEqual(["a b"]);
  });

  it("gives an over-wide word its own line instead of dropping it", () => {
    expect(wrapWords(["short", "enormousword"], 5, (line) => line.length)).toEqual([
      "short",
      "enormousword",
    ]);
  });
});

describe("layoutCard", () => {
  it("draws the verse text verbatim on the Arabic face, right to left", () => {
    const layout = layoutCard(input(), measure);
    const body = layout.lines.filter((line) => line.font.includes("Amiri"));
    expect(body.every((line) => line.rtl)).toBe(true);
    expect(body.map((line) => line.text).join(" ")).toBe(AYAH);
  });

  it("prints the reference and link only when asked", () => {
    const withRef = layoutCard(input(), measure).lines.map((line) => line.text);
    expect(withRef).toContain("Al-Baqarah 2:255");
    expect(withRef).toContain("https://easyquran.test/al-baqarah?ayah=2:255");

    const bare = layoutCard(input({ reference: false, link: null }), measure).lines.map(
      (line) => line.text,
    );
    expect(bare).not.toContain("Al-Baqarah 2:255");
    expect(bare.some((text) => text.startsWith("https://"))).toBe(false);
    expect(bare).toContain("Read on EasyQuran");
  });

  it("uses the card width and grows height with more verses", () => {
    const one = layoutCard(input(), measure);
    const two = layoutCard(
      input({
        verses: [
          { text: AYAH, ref: "Al-Baqarah 2:255" },
          { text: AYAH, ref: "Al-Baqarah 2:256" },
        ],
      }),
      measure,
    );
    expect(one.width).toBe(CARD_WIDTH);
    expect(two.height).toBeGreaterThan(one.height);
  });
});
