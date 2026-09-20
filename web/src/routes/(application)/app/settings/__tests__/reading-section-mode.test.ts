import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => {
  const setModeSpy = vi.fn();
  const entry = (id: string, language: string, languageCode: string) => ({
    id,
    language,
    languageCode,
    direction: "ltr" as const,
    name: `Name ${id}`,
    translator: `Translator ${id}`,
    sizeBytes: 2048,
    downloadUrl: "",
  });
  return {
    catalogue: [entry("en.sahih", "English", "en"), entry("en.arberry", "English", "en")],
    gotoSpy: vi.fn().mockResolvedValue(undefined),
    resumeVerseSpy: vi.fn().mockResolvedValue(true),
    resumeLastReadSpy: vi.fn(),
    setModeSpy,
    setSourceIdSpy: vi.fn(),
    // SAFETY: null is the not-yet-seeded member; tests reassign concrete catalogue ids.
    sourceState: { sourceId: null as string | null },
    readerStub: {
      mode: "verse",
      isReadingMode: false,
      isVerseMode: true,
      hasLastRead: true,
      // SAFETY: null is a member of the anchor union; no test in this file sets a concrete anchor.
      lastReadAnchor: null,
      // last read on a DIFFERENT source than the confirmed candidate, to pin that the candidate wins.
      lastRead: { num: 2, n: 5, sourceId: "en.sahih" },
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

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$app/navigation", () => ({ goto: h.gotoSpy }));
vi.mock("$lib/stores/reader.svelte", () => ({
  reader: h.readerStub,
  ReaderMode: { Reading: "reading", Verse: "verse" },
}));
vi.mock("$lib/stores/reader-settings.svelte", () => ({
  readerSource: {
    get sourceId() {
      return h.sourceState.sourceId;
    },
    setSourceId: h.setSourceIdSpy,
  },
}));
vi.mock("$lib/quran/engagement", () => ({
  noteTranslationChosen: vi.fn(() => Promise.resolve()),
}));
vi.mock("$lib/reader/resume", () => ({
  resumeToVerse: h.resumeVerseSpy,
  resumeToLastRead: h.resumeLastReadSpy,
}));
vi.mock("$lib/fonts/arabic-fonts", () => ({ loadArabicFont: vi.fn() }));
vi.mock("$lib/quran/catalogue", () => ({
  TRANSLATION_CATALOGUE_BY_ID: new Map(h.catalogue.map((t) => [t.id, t])),
  flagFor: () => ({ flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" }),
  translationSourceOf: () => "tanzil",
}));

import { getSettingsCopy } from "$lib/i18n/settings-copy";
import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
import { readingModeUi } from "../../_reader/reading-mode-guard.svelte";
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
  h.setSourceIdSpy.mockClear();
  h.sourceState.sourceId = null;
  h.gotoSpy.mockClear();
  h.resumeVerseSpy.mockClear();
  h.resumeVerseSpy.mockResolvedValue(true);
  readingModeUi.reset();
  stackedTranslations.clear();
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
  for (const el of document.querySelectorAll("[data-dialog-content], [data-dialog-overlay]")) {
    el.remove();
  }
});

function mountSection(): void {
  instance = mount(ReadingSection, { target, props: { id: "reading", heading: "Reading", copy: readingCopy } });
}

function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

function readingPill(): HTMLButtonElement {
  // SAFETY: the reading pill is the mode button whose label is the reading mode name
  const pill = [...document.querySelectorAll("button")].find((b) =>
    b.textContent?.includes(readingCopy.modeNames.reading),
  );
  // SAFETY: the query matches only button elements, so the found value is an HTMLButtonElement.
  return pill as HTMLButtonElement;
}

function radios(): HTMLInputElement[] {
  // SAFETY: selector matches only radio inputs, so every element is HTMLInputElement
  return [...document.querySelectorAll('input[type="radio"]')] as HTMLInputElement[];
}

function confirmButton(): HTMLButtonElement {
  // SAFETY: selector matches only the dialog's confirm button
  return document.querySelector("button[data-reading-confirm]") as HTMLButtonElement;
}

describe("ReadingSection mode pill reading-mode-guard interception", () => {
  it("confirms before switching to reading when translations are in play; mode unchanged until then", async () => {
    h.sourceState.sourceId = "en.sahih";
    stackedTranslations.setIds(["en.sahih", "en.arberry"]);
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    expect(radios().map((r) => r.value)).toEqual(["en.sahih", "en.arberry"]);
    expect(h.readerStub.mode).toBe("verse");
  });

  it("cancel leaves the mode untouched", async () => {
    h.sourceState.sourceId = "en.sahih";
    stackedTranslations.setIds(["en.sahih", "en.arberry"]);
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    const cancel = [...document.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Cancel"),
    );
    cancel?.click();
    await settle();
    expect(h.readerStub.mode).toBe("verse");
    expect(h.setSourceIdSpy).not.toHaveBeenCalled();
  });

  it("confirming another translation resumes at the last-read position instead of a raw goto", async () => {
    h.sourceState.sourceId = "en.sahih";
    stackedTranslations.setIds(["en.sahih", "en.arberry"]);
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    radios()[1]?.click();
    confirmButton().click();
    expect(h.readerStub.mode).toBe("reading");
    expect(readingModeUi.appliedByUi).toBe(true);
    expect(h.setSourceIdSpy).toHaveBeenCalledWith("en.arberry");
    // The CHOSEN candidate pins the resume context, not lastRead.sourceId ("en.sahih").
    expect(h.resumeVerseSpy).toHaveBeenCalledWith(
      2,
      5,
      "en.arberry",
      { kind: "translation", lang: "en", translator: "arberry" },
      { anchor: null },
    );
    // the lastRead-preferring helper must NOT be used for candidate confirms
    expect(h.resumeLastReadSpy).not.toHaveBeenCalled();
    expect(h.gotoSpy).not.toHaveBeenCalled();
  });

  it("confirming the current primary applies reading mode in place", async () => {
    h.sourceState.sourceId = "en.sahih";
    stackedTranslations.setIds(["en.sahih"]);
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    confirmButton().click();
    expect(h.readerStub.mode).toBe("reading");
    expect(h.setSourceIdSpy).not.toHaveBeenCalled();
    expect(h.resumeVerseSpy).not.toHaveBeenCalled();
  });

  it("switches directly (no dialog) for an Arabic source with zero extras", async () => {
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    expect(h.readerStub.mode).toBe("reading");
    expect(radios()).toHaveLength(0);
  });

  it("switching back to verse resets the UI-gesture flag", async () => {
    mountSection();
    await settle();
    readingPill().click();
    await settle();
    expect(h.readerStub.mode).toBe("reading");
    h.readerStub.mode = "verse";
    h.readerStub.isReadingMode = false;
    const versePill = [...document.querySelectorAll("button")].find((b) =>
      b.textContent?.includes(readingCopy.modeNames.verse),
    );
    versePill?.click();
    expect(readingModeUi.appliedByUi).toBe(false);
  });
});
