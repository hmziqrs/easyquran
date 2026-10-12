import { mount } from "svelte";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { nav } = vi.hoisted(() => ({
  // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
  nav: { url: { hash: "" }, params: {} as Record<string, string> },
}));

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: nav }));

import ReadingAyah from "../ReadingAyah.svelte";

let target: HTMLElement;

beforeEach(() => {
  nav.params = {};
  target = document.createElement("div");
  document.body.appendChild(target);
});

describe("ReadingAyah translation rendering (TRX-02, TRX-11, EC-45)", () => {
  it("shows the empty-verse note under the declared direction with the catalogue name", () => {
    // ku.asan (108:3 is one of the four known upstream blanks) declares rtl.
    nav.params = { lang: "ku", translator: "asan" };
    mount(ReadingAyah, {
      target,
      props: { text: "", n: 3, vKey: "108:3", isTranslation: true, translationLang: "ku" },
    });
    // SAFETY: the ayah span renders exactly one translation line
    const line = target.querySelector(".translation-text") as HTMLElement;
    // TRX-11: dir is the catalogue-declared direction, never a dir="auto" guess.
    expect(line.getAttribute("dir")).toBe("rtl");
    // TRX-15: the name comes from the baked catalogue row, never URL segments.
    expect(line.querySelector(".verse-missing")?.textContent).toContain(
      "No translation for this verse in ته‌فسیری ئاسان.",
    );
  });

  it("parses transliteration marks into styled runs with the legend, never raw tags", () => {
    mount(ReadingAyah, {
      target,
      props: {
        text: "Bismi All<u>a</U>hi a<b>l</B>rra<u>h</U>m",
        n: 1,
        vKey: "1:1",
        isTranslation: true,
      },
    });
    // SAFETY: the ayah span renders exactly one translation line
    const line = target.querySelector(".translation-text") as HTMLElement;
    // Off the /t/** routes there is no route translation, so direction stays auto.
    expect(line.getAttribute("dir")).toBe("auto");
    expect(line.textContent).not.toContain("<");
    expect(line.querySelector("u.translit-long")?.textContent).toBe("a");
    expect(line.querySelector("b.translit-assimilated")?.textContent).toBe("l");
    expect(line.querySelector(".translit-legend")?.textContent).toContain("Underlined");
  });

  it("keeps the Arabic reading line untouched (no note, no legend)", () => {
    mount(ReadingAyah, { target, props: { text: "بِسْمِ", n: 1, vKey: "1:1" } });
    expect(target.querySelector(".arabic-text")).not.toBeNull();
    expect(target.querySelector(".verse-missing")).toBeNull();
    expect(target.querySelector(".translit-legend")).toBeNull();
  });
});
