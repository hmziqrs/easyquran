import { mount } from "svelte";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

const { nav, readerStub, toolsCalls } = vi.hoisted(() => ({
  // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
  nav: { url: { hash: "" }, params: {} as Record<string, string> },
  readerStub: {
    isVerseMode: true,
    isReadingMode: false,
    // SAFETY: null seeds the nullable openNote union; no test assigns before the row reads it
    openNote: null as string | null,
  },
  // The dynamically imported verse tools land here as a bare function component
  // (Svelte 5 calls it with (anchor, props)), so the copy path is assertable.
  // SAFETY: only the mocked VerseTools pushes here, always with the exact
  // { text, vKey } props VerseRow passes; the empty literal just needs widening.
  toolsCalls: [] as { text: string; vKey: string }[],
}));

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: nav }));
vi.mock("#lib/stores/reader.svelte.js", () => ({ reader: readerStub }));
vi.mock("../VerseTools.svelte", () => ({
  default: (_anchor: Node, props: { text: string; vKey: string }) => {
    toolsCalls.push(props);
  },
}));

import type { StackedTranslation } from "#lib/data/quran-types.js";

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
  toolsCalls.length = 0;
  target = document.createElement("div");
  document.body.appendChild(target);
});

const extras = (): Element[] => [...target.querySelectorAll(".verse-extra")];

describe("VerseRow stacked extras", () => {
  it("renders ready extras with the BCP-47 lang code and the catalogue name (translator as fallback)", () => {
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
    // dir is the lane's declared direction (TRX-11), never a guess
    expect(rows[0]?.getAttribute("dir")).toBe("ltr");
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

  it("credits each lane after its text: translator, then the work when named differently", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [
          stacked("qul.r158.bayan-ul-quran", {
            name: "Bayan-ul-Quran",
            translator: "Dr. Israr Ahmad",
            direction: "rtl",
          }),
          stacked("en.sahih", { name: "Saheeh International", translator: "Saheeh International" }),
        ],
      },
    });
    const credits = [...target.querySelectorAll(".verse-extra-credit")].map((el) =>
      el.textContent?.trim(),
    );
    expect(credits).toEqual(["— Dr. Israr Ahmad (Bayan-ul-Quran)", "— Saheeh International"]);
    expect(target.querySelector(".verse-extra-credit")?.getAttribute("dir")).toBe("rtl");
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

  it("renders a stacked tafsir as a distinct commentary block labelled commentary (TRX-01)", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 1,
        vKey: "1:1",
        stacked: [
          stacked("ar.muyassar", {
            language: "Arabic",
            languageCode: "ar",
            direction: "rtl",
            name: "Tafsir al-Muyassar",
            translator: "King Fahad Quran Complex",
          }),
        ],
      },
    });
    const lane = extras()[0];
    expect(lane?.classList.contains("verse-extra--tafsir")).toBe(true);
    expect(lane?.getAttribute("dir")).toBe("rtl");
    const credit = lane?.querySelector(".verse-extra-credit");
    expect(credit?.textContent).toContain("King Fahad Quran Complex (Tafsir al-Muyassar)");
    expect(credit?.textContent).toContain("(commentary)");
  });

  it("shows the empty-verse note with the declared direction instead of an empty lane (TRX-11, EC-45)", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "arabic",
        n: 3,
        vKey: "108:3",
        stacked: [
          stacked("ku.asan", {
            language: "Kurdish",
            languageCode: "ku",
            direction: "rtl",
            text: "",
          }),
        ],
      },
    });
    const lane = extras()[0];
    expect(lane?.getAttribute("dir")).toBe("rtl");
    const missing = lane?.querySelector(".verse-missing");
    expect(missing?.textContent).toContain("No translation for this verse in Kurdish.");
    expect(missing?.textContent).not.toContain("<");
  });

  it("parses transliteration markup into styled runs and never prints raw tags (TRX-02)", () => {
    mount(VerseRow, {
      target,
      props: {
        text: "Bismi All<u>a</U>hi a<b>l</B>rra<u>h</U>m",
        n: 1,
        vKey: "1:1",
        isTranslation: true,
      },
    });
    // SAFETY: the translation line is the row's only verse-text--translation span
    const line = target.querySelector(".verse-text--translation") as HTMLElement;
    expect(line.textContent).not.toContain("<");
    expect(line.querySelector("u.translit-long")?.textContent).toBe("a");
    expect(line.querySelector("b.translit-assimilated")?.textContent).toBe("l");
    expect(target.querySelector(".translit-legend")?.textContent).toContain("Underlined");
  });
});

describe("VerseRow copy path (TRX-02 markup stripped, EC-45 placeholder stripped)", () => {
  it("hands verse tools the text without transliteration tags", async () => {
    mount(VerseRow, {
      target,
      props: {
        text: "Bismi All<u>a</U>hi a<b>l</B>rra<u>h</U>m",
        n: 1,
        vKey: "1:1",
        isTranslation: true,
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(toolsCalls.at(-1)?.text).toBe("Bismi Allahi alrrahm");
  });

  it("copies no placeholder text for an empty translation verse", async () => {
    nav.params = { lang: "ku", translator: "asan" };
    mount(VerseRow, {
      target,
      props: { text: "", n: 3, vKey: "108:3", isTranslation: true },
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(toolsCalls.at(-1)?.text).toBe("");
    expect(JSON.stringify(toolsCalls)).not.toContain("No translation");
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
