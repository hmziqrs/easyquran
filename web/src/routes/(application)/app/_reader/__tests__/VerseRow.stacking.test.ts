import { mount } from "svelte";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { nav, readerStub, mountStub } = vi.hoisted(() => ({
  // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
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

import type { StackedTranslation } from "$lib/data/quran-types";
import VerseRow from "../VerseRow.svelte";

const stacked = (
  sourceId: string,
  overrides: Partial<StackedTranslation> = {},
): StackedTranslation => ({
  sourceId,
  translator: `T-${sourceId}`,
  language: "English",
  languageCode: "en",
  direction: "ltr",
  text: `text-${sourceId}`,
  ...overrides,
});

let target: HTMLElement;

beforeEach(() => {
  readerStub.isVerseMode = true;
  readerStub.isReadingMode = false;
  readerStub.openNote = null;
  nav.params = {};
  target = document.createElement("div");
  document.body.appendChild(target);
});

const extras = (): Element[] => [...target.querySelectorAll(".verse-extra")];

describe("VerseRow stacked extras", () => {
  it("renders ready extras in store order with the BCP-47 lang code and translator label", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [
          stacked("en.sahih"),
          stacked("ur.jalandhry", { languageCode: "ur", direction: "rtl", language: "Urdu" }),
        ],
      },
    });
    const rows = extras();
    expect(rows).toHaveLength(2);
    expect(rows[0]?.getAttribute("lang")).toBe("en");
    expect(rows[0]?.getAttribute("dir")).toBe("auto");
    expect(rows[0]?.textContent).toContain("T-en.sahih");
    expect(rows[0]?.textContent).toContain("text-en.sahih");
    expect(rows[1]?.getAttribute("lang")).toBe("ur");
    expect(rows[1]?.getAttribute("dir")).toBe("rtl");
  });

  it("falls back to the language name when translator is null", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [stacked("en.sahih", { translator: null, language: "English" })],
      },
    });
    expect(extras()[0]?.textContent).toContain("English");
    expect(extras()[0]?.textContent).toContain("text-en.sahih");
  });

  it("renders skeletons for pending sources and an error row for errored ones", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [stacked("en.sahih")],
        stackedPending: ["ur.jalandhry"],
        stackedErrored: ["fr.hamid"],
        stackedErrorLabel: "Failed to load",
      },
    });
    const skeletons = [...target.querySelectorAll(".verse-extra--skeleton")];
    expect(skeletons).toHaveLength(1);
    expect(skeletons[0]?.getAttribute("aria-hidden")).toBe("true");
    const error = target.querySelector(".verse-extra--error");
    expect(error?.textContent).toBe("Failed to load");
    expect(target.querySelectorAll(".verse-extra")).toHaveLength(3);
  });

  it("renders no extras at all in reading mode (DOM-absent, not CSS-hidden)", () => {
    readerStub.isVerseMode = false;
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [stacked("en.sahih")],
        stackedPending: ["ur.jalandhry"],
        stackedErrored: ["fr.hamid"],
      },
    });
    expect(extras()).toHaveLength(0);
  });
});

describe("VerseRow translation reading flow (U10)", () => {
  it("joins the continuous flow in translation reading mode and keeps the anchor id", () => {
    readerStub.isVerseMode = false;
    readerStub.isReadingMode = true;
    mount(VerseRow, {
      target,
      props: { text: "In the name of God", n: 2, vKey: "1:2", isTranslation: true },
    });
    // SAFETY: the row list always renders exactly one li, the verse row under test
    const row = target.querySelector("li") as HTMLElement;
    expect(row.className).toContain("verse-row--translation-flow");
    // #ayah deep-link anchors survive the inline flow
    expect(row.id).toBe("ayah-1-2");
  });

  it("keeps verse-mode translation rows as block rows (no flow class)", () => {
    mount(VerseRow, {
      target,
      props: { text: "In the name of God", n: 2, vKey: "1:2", isTranslation: true },
    });
    // SAFETY: the row list always renders exactly one li, the verse row under test
    const row = target.querySelector("li") as HTMLElement;
    expect(row.className).not.toContain("verse-row--translation-flow");
  });

  it("leaves the Arabic reading flow untouched (no translation flow class)", () => {
    readerStub.isVerseMode = false;
    readerStub.isReadingMode = true;
    mount(VerseRow, { target, props: { text: "بسم الله", n: 1, vKey: "1:1" } });
    // SAFETY: the row list always renders exactly one li, the verse row under test
    const row = target.querySelector("li") as HTMLElement;
    expect(row.className).not.toContain("verse-row--translation-flow");
  });
});
