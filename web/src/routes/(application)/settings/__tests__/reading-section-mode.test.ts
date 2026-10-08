import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => {
  const setModeSpy = vi.fn();
  return {
    gotoSpy: vi.fn().mockResolvedValue(undefined),
    setModeSpy,
    readerStub: {
      mode: "verse",
      isReadingMode: false,
      isVerseMode: true,
      arabicScript: "uthmani",
      arabicFont: "amiri",
      arabicSizePx: "33px",
      translationFamily: "sans",
      translationSizePx: "17px",
      setArabicFont: vi.fn(),
      setArabicScript: vi.fn(),
      setTranslationFamily: vi.fn(),
      smaller: vi.fn(),
      bigger: vi.fn(),
      shrinkTranslation: vi.fn(),
      growTranslation: vi.fn(),
      setMode: setModeSpy,
    },
    nav: {
      // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
      params: {} as Record<string, string>,
      state: {},
    },
  };
});

vi.mock("$app/env", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$app/navigation", () => ({ goto: h.gotoSpy }));
vi.mock("#lib/stores/reader.svelte.js", () => ({
  reader: h.readerStub,
  ReaderMode: { Reading: "reading", Verse: "verse" },
}));
vi.mock("#lib/fonts/arabic-fonts.js", () => ({ loadArabicFont: vi.fn() }));

import { getSettingsCopy } from "#lib/i18n/settings-copy.js";
import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";

import ReadingSection from "../_components/ReadingSection.svelte";

const readingCopy = getSettingsCopy("en").reading;

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  h.readerStub.mode = "verse";
  h.readerStub.isReadingMode = false;
  h.readerStub.isVerseMode = true;
  h.setModeSpy.mockClear();
  h.setModeSpy.mockImplementation((m: string) => {
    h.readerStub.mode = m;
    h.readerStub.isReadingMode = m === "reading";
    h.readerStub.isVerseMode = m === "verse";
  });
  h.gotoSpy.mockClear();
  stackedTranslations.clear();
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
});

function mountSection(): void {
  instance = mount(ReadingSection, {
    target,
    props: { id: "reading", heading: "Reading", copy: readingCopy },
  });
}

function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

function modePill(name: string): HTMLButtonElement {
  const pill = [...document.querySelectorAll("button")].find((b) => b.textContent?.includes(name));
  // SAFETY: the query matches only button elements, so the found value is an HTMLButtonElement.
  return pill as HTMLButtonElement;
}

describe("ReadingSection mode pills", () => {
  it("switches to reading at once even with translations stacked — no confirmation, no navigation", async () => {
    stackedTranslations.setIds(["en.sahih", "en.arberry"]);
    mountSection();
    await settle();
    modePill(readingCopy.modeNames.reading).click();
    await settle();
    expect(h.setModeSpy).toHaveBeenCalledWith("reading");
    expect(h.readerStub.mode).toBe("reading");
    expect(document.querySelectorAll('input[type="radio"]')).toHaveLength(0);
    expect(h.gotoSpy).not.toHaveBeenCalled();
  });

  it("switches back to verse mode the same way", async () => {
    h.readerStub.mode = "reading";
    h.readerStub.isReadingMode = true;
    h.readerStub.isVerseMode = false;
    mountSection();
    await settle();
    modePill(readingCopy.modeNames.verse).click();
    await settle();
    expect(h.setModeSpy).toHaveBeenCalledWith("verse");
    expect(h.readerStub.mode).toBe("verse");
  });
});
