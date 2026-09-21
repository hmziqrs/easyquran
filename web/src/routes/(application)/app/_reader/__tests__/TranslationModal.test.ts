import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => {
  const entry = (
    id: string,
    language: string,
    languageCode: string,
    direction: "ltr" | "rtl",
    name: string,
    translator: string | null,
  ) => ({
    id,
    language,
    languageCode,
    direction,
    name,
    translator,
    sizeBytes: 2048,
    downloadUrl: "",
  });
  return {
    catalogue: [
      entry("ar.muyassar", "Arabic", "ar", "rtl", "Al-Muyassar", "Al-Muyassar"),
      entry("en.sahih", "English", "en", "ltr", "Saheeh International", "Saheeh International"),
      entry("en.pickthall", "English", "en", "ltr", "Pickthall", "Marmaduke Pickthall"),
      entry("qul.en.ahmed", "English", "en", "ltr", "Ahmed Ali", "Ahmed Ali"),
      entry("en.wahiduddin", "English", "en", "ltr", "Wahiduddin Khan", "Wahiduddin Khan"),
      entry("ur.jalandhry", "Urdu", "ur", "rtl", "Jalandhry", "Maulana Jalal ad-Din"),
      entry("qul.ur.bayan", "Urdu", "ur", "rtl", "Bayan-ul-Quran", "Dr. Israr Ahmad"),
      entry("ms.basmeih", "Malay", "ms", "ltr", "Basmeih", "Abdullah Muhammad Basmeih"),
      entry("quranenc.fr.hamidullah", "French", "fr", "ltr", "Hamidullah", "Muhammad Hamidullah"),
      entry("tr.diyanet", "Turkish", "tr", "ltr", "Diyanet", "Diyanet Isleri"),
      entry("id.indonesian", "Indonesian", "id", "ltr", "Kemenag", "Kemenag"),
      entry("es.cortes", "Spanish", "es", "ltr", "Cortés", "Hernán Cortés"),
    ],
    nav: {
      url: new URL("https://example.test/app/al-fatihah"),
      // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
      params: {} as Record<string, string>,
      state: {},
    },
    replaceState: vi.fn(),
    setSourceId: vi.fn(),
    readerStub: {
      isVerseMode: true,
      isReadingMode: false,
    },
  };
});

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$app/navigation", () => ({ replaceState: h.replaceState }));
vi.mock("$lib/stores/reader.svelte", () => ({ reader: h.readerStub }));
vi.mock("$lib/stores/reader-settings.svelte", () => ({
  readerSource: { sourceId: null, setSourceId: h.setSourceId },
}));
vi.mock("$lib/quran/engagement", () => ({
  noteTranslationChosen: vi.fn(() => Promise.resolve()),
}));
vi.mock("$lib/paraglide/runtime", async (importOriginal) => {
  const actual = await importOriginal<typeof import("$lib/paraglide/runtime")>();
  // Mirrors the real deLocalizeUrl contract (string | URL in, URL out) — the
  // live-position helper passes window.location.href as a string.
  return { ...actual, deLocalizeUrl: (url: URL | string) => new URL(url) };
});
vi.mock("$lib/hotkeys.svelte", () => ({
  registerHotkey: () => ({ unregister: () => {} }),
}));
vi.mock("$lib/quran/catalogue", () => {
  const byId = new Map(h.catalogue.map((t) => [t.id, t]));
  const translationSourceOf = (id: string): string => {
    if (id.startsWith("qul.")) return "qul";
    if (id.startsWith("quranenc.")) return "quranenc";
    return "tanzil";
  };
  // Mirrors the real flagFor contract; the real map's catalogue coverage is
  // pinned separately in language-flags.test.ts (unmocked).
  const flags = new Map([
    ["ar", { flag: "\u{1F1F8}\u{1F1E6}", country: "Saudi Arabia" }],
    ["en", { flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" }],
    ["ur", { flag: "\u{1F1F5}\u{1F1F0}", country: "Pakistan" }],
    ["ms", { flag: "\u{1F1F2}\u{1F1FE}", country: "Malaysia" }],
    ["fr", { flag: "\u{1F1EB}\u{1F1E7}", country: "France" }],
    ["tr", { flag: "\u{1F1F9}\u{1F1F7}", country: "Turkey" }],
    ["id", { flag: "\u{1F1EE}\u{1F1E9}", country: "Indonesia" }],
    ["es", { flag: "\u{1F1EA}\u{1F1F8}", country: "Spain" }],
  ]);
  // Mirrors the real nativeNameFor contract; the real map's catalogue coverage
  // is pinned separately in language-autonyms.test.ts (unmocked).
  const autonyms = new Map([
    ["ar", "العربية"],
    ["ur", "اردو"],
    ["fr", "Français"],
    ["tr", "Türkçe"],
    ["id", "Bahasa Indonesia"],
    ["es", "Español"],
  ]);
  return {
    TRANSLATION_CATALOGUE: h.catalogue,
    TRANSLATION_CATALOGUE_BY_ID: byId,
    translationSourceOf,
    flagFor: (code: string) =>
      flags.get(code) ?? { flag: "\u{1F310}", country: "" },
    nativeNameFor: (code: string) => autonyms.get(code) ?? null,
  };
});

import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
import TranslationButton from "../TranslationButton.svelte";
import TranslationModal from "../TranslationModal.svelte";
import TranslationModalHost from "./TranslationModalHost.svelte";

const FILLER_IDS = [
  "en.pickthall",
  "ur.jalandhry",
  "ms.basmeih",
  "quranenc.fr.hamidullah",
  "tr.diyanet",
];

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;

// jsdom ships no scrollIntoView; patch it on the prototype so the rail's
// keep-selected-visible effect can be observed in tests.
const originalScrollIntoView: ((arg?: boolean | ScrollIntoViewOptions) => void) | undefined =
  Element.prototype.scrollIntoView?.bind(Element.prototype);
let scrolledIntoView: Element[] = [];

beforeEach(() => {
  h.readerStub.isVerseMode = true;
  h.readerStub.isReadingMode = false;
  h.nav.url = new URL("https://example.test/app/al-fatihah");
  h.replaceState.mockClear();
  h.setSourceId.mockClear();
  localStorage.clear();
  stackedTranslations.clear();
  scrolledIntoView = [];
  Element.prototype.scrollIntoView = function scrollSpy(this: Element): void {
    scrolledIntoView.push(this);
  };
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
  // SAFETY: jsdom omits scrollIntoView entirely, so the saved original can be undefined at runtime while the DOM types require it.
  Element.prototype.scrollIntoView = originalScrollIntoView as typeof Element.prototype.scrollIntoView;
  // bits-ui portals can leave dialog shells behind across mounts; clear them so
  // document-level queries only ever see the current modal under test.
  for (const el of document.querySelectorAll("[data-dialog-content], [data-dialog-overlay]")) {
    el.remove();
  }
});

const railOptions = (): HTMLButtonElement[] => {
  // SAFETY: selector matches only the rail option buttons
  return [...document.querySelectorAll("[data-language-option]")] as HTMLButtonElement[];
};
const railOption = (language: string): HTMLButtonElement | undefined =>
  railOptions().find((b) => b.getAttribute("data-language-option") === language);
const railNames = (): string[] =>
  railOptions().map((b) => b.querySelector("[data-language-name]")?.textContent ?? "");
const rail = (): HTMLElement => {
  // SAFETY: the rail nav is present whenever the modal is open
  return document.querySelector("[data-language-rail]") as HTMLElement;
};
const pane = (): HTMLElement => {
  // SAFETY: the pane section is present whenever the modal is open
  return document.querySelector("[data-language-pane]") as HTMLElement;
};
const checkboxes = (): HTMLInputElement[] => {
  // SAFETY: selector matches only checkbox inputs, so every element is HTMLInputElement
  return [...document.querySelectorAll('input[type="checkbox"]')] as HTMLInputElement[];
};
const switchLinks = (): HTMLAnchorElement[] => {
  // SAFETY: selector matches only anchor elements, so every element is HTMLAnchorElement
  return [...document.querySelectorAll("a[data-sveltekit-preload-data]")] as HTMLAnchorElement[];
};
const searchInput = (): HTMLInputElement => {
  // SAFETY: selector matches only the modal's search input, which is an HTMLInputElement
  return document.querySelector('input[type="search"]') as HTMLInputElement;
};
const paneRowIds = (): string[] =>
  [...pane().querySelectorAll("li[data-translation-row]")].map((li) =>
    li.getAttribute("data-translation-row") ?? "",
  );

// bits-ui portals dialog content in after mount (presence transition) — let a
// macrotask tick land before querying the portaled DOM.
function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

async function setSearch(query: string): Promise<void> {
  const input = searchInput();
  input.value = query;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await settle();
}

async function open(props: { primaryId?: string | null } = {}): Promise<void> {
  instance = mount(TranslationModal, {
    target,
    props: { open: true, primaryId: props.primaryId ?? null },
  });
  await settle();
}

describe("TranslationModal — master-detail layout", () => {
  it("renders the language rail with Arabic pinned, English second, then the alphabetical tail", async () => {
    await open();
    // jsdom's default navigator.languages is ["en-US"], so the only boost
    // (en) is already pinned — the visible order is Arabic, English, then
    // alphabetical (U21 priority sort).
    expect(railNames()).toEqual([
      "Arabic",
      "English",
      "French",
      "Indonesian",
      "Malay",
      "Spanish",
      "Turkish",
      "Urdu",
    ]);
    const english = railOption("English");
    expect(english?.textContent).toContain("\u{1F1EC}\u{1F1E7}");
    expect(english?.textContent).toContain("4");
    // U17 height must survive the rail's flex column: flex-none keeps the
    // rows from shrinking below h-[52px] to min-content (vision r7 item 36).
    expect(english?.className).toContain("h-[52px]");
    expect(english?.className).toContain("flex-none");
  });

  it("boosts the user's browser languages after Arabic and English", async () => {
    // SAFETY: configurable in jsdom so the stub restores cleanly; the modal
    // only reads navigator.languages inside onMount (client-only, SSR-safe).
    const originalLanguages = navigator.languages;
    Object.defineProperty(navigator, "languages", {
      value: ["tr", "ur-PK", "zz-XX"],
      configurable: true,
    });
    try {
      await open();
      expect(railNames()).toEqual([
        "Arabic",
        "English",
        "Turkish",
        "Urdu",
        "French",
        "Indonesian",
        "Malay",
        "Spanish",
      ]);
    } finally {
      Object.defineProperty(navigator, "languages", {
        value: originalLanguages,
        configurable: true,
      });
    }
  });

  it("shows the native autonym as a muted secondary under rail language names", async () => {
    await open();
    const urdu = railOption("Urdu");
    const autonym = urdu?.querySelector("[data-language-autonym]");
    expect(autonym?.textContent).toBe("\u0627\u0631\u062F\u0648");
    expect(autonym?.getAttribute("dir")).toBe("auto");
    // English has no distinct autonym: the documented fallback renders no
    // secondary line instead of duplicating the English name.
    expect(railOption("English")?.querySelector("[data-language-autonym]")).toBeNull();
    // RTL autonym still renders inside the rail row
    expect(railOption("Arabic")?.querySelector("[data-language-autonym]")?.textContent).toBe(
      "\u0627\u0644\u0639\u0631\u0628\u064A\u0629",
    );
  });

  it("auto-selects the primary translation's language on open", async () => {
    await open({ primaryId: "qul.ur.bayan" });
    const active = railOptions().find((b) => b.getAttribute("aria-current") === "true");
    expect(active?.getAttribute("data-language-option")).toBe("Urdu");
    // the pane header names the language with a quiet count
    expect(pane().querySelector("h3")?.textContent).toContain("Urdu");
    expect(paneRowIds()).toEqual(["qul.ur.bayan", "ur.jalandhry"]);
  });

  it("scrolls the selected rail row into view (open auto-select and keyboard moves)", async () => {
    await open({ primaryId: "qul.ur.bayan" });
    // Urdu sits last in the fixture rail: the auto-selection must be revealed
    // instead of resting below the fold.
    const scrolledOnOpen = scrolledIntoView.map((el) => el.getAttribute("data-language-option"));
    expect(scrolledOnOpen).toContain("Urdu");

    const english = railOption("English");
    english?.focus();
    english?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    );
    await settle();
    const scrolledOnKey = scrolledIntoView.map((el) => el.getAttribute("data-language-option"));
    expect(scrolledOnKey).toContain("French");
  });

  it("pluralizes the count line: 4 translations vs 1 translation", async () => {
    await open({ primaryId: "en.sahih" });
    expect(document.querySelector("[data-results-count]")?.textContent?.trim()).toBe(
      "4 translations",
    );
    railOption("Turkish")?.click();
    await settle();
    expect(document.querySelector("[data-results-count]")?.textContent?.trim()).toBe(
      "1 translation",
    );
  });

  it("selecting a rail language swaps the pane rows", async () => {
    await open();
    railOption("Turkish")?.click();
    await settle();
    expect(paneRowIds()).toEqual(["tr.diyanet"]);
    expect(pane().querySelector("h3")?.textContent).toContain("Turkish");
  });

  it("moves keyboard focus through the rail with arrow keys", async () => {
    await open({ primaryId: "en.sahih" });
    const english = railOption("English");
    english?.focus();
    expect(document.activeElement).toBe(english);
    english?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    );
    await settle();
    const french = railOption("French");
    expect(document.activeElement).toBe(french);
    expect(french?.getAttribute("aria-current")).toBe("true");
    french?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(english);
    expect(english?.getAttribute("aria-current")).toBe("true");
  });

  it("uses one row anatomy: name first, author line only when it differs", async () => {
    await open({ primaryId: "en.sahih" });
    const pickthall = pane().querySelector('li[data-translation-row="en.pickthall"]');
    expect(pickthall?.textContent).toContain("Pickthall");
    expect(pickthall?.querySelector("[data-author-line]")?.textContent).toContain(
      "Marmaduke Pickthall",
    );
    // translator mirrors the name: no duplicated second line
    const sahih = pane().querySelector('li[data-translation-row="en.sahih"]');
    expect(sahih?.querySelector("[data-author-line]")).toBeNull();
    // every row carries an 18px checkbox and is itself the rich-tooltip
    // trigger (U16: hover anywhere / keyboard focus on the row opens it)
    expect(pickthall?.querySelector('input[type="checkbox"]')?.className).toContain("size-[18px]");
    expect(pickthall?.hasAttribute("data-tooltip-trigger")).toBe(true);
    // U19: no colored source dot anywhere in the rows — provenance is
    // tooltip-only now
    expect(document.querySelectorAll("li[data-translation-row] span.size-2")).toHaveLength(0);
  });

  it("shows the Primary badge instead of a checkbox on the primary row", async () => {
    await open({ primaryId: "en.sahih" });
    const sahih = pane().querySelector('li[data-translation-row="en.sahih"]');
    expect(sahih?.querySelector('input[type="checkbox"]')).toBeNull();
    expect(sahih?.textContent).toContain("Primary");
    // non-primary rows stay toggleable
    expect(checkboxes().length).toBe(3);
  });

  it("keeps the whole-wrapper search focus ring and offers a clear button", async () => {
    await open();
    const input = searchInput();
    const wrapper = input.closest("div");
    expect(wrapper?.className).toContain("focus-within:border-border-strong");
    expect(input.className).not.toContain("focus-visible:outline-2");
    expect(document.querySelector('button[aria-label="Clear search"]')).toBeNull();
    await setSearch("urdu");
    // SAFETY: selector matches only the search field's clear button element
    const clear = document.querySelector(
      'button[aria-label="Clear search"]',
    ) as HTMLButtonElement | null;
    expect(clear).toBeTruthy();
    clear?.click();
    await settle();
    expect(searchInput().value).toBe("");
    expect(railOptions()).toHaveLength(8);
  });

  it.each([
    ["urdu", "Urdu", 2],
    ["englsh", "English", 4],
    ["israr", "Urdu", 1],
    ["khan", "English", 1],
    ["turkey", "Turkish", 1],
    ["cortes", "Spanish", 1],
  ])("fuzzy search %s finds %i flat cross-language row(s)", async (query, _language, rowCount) => {
    await open();
    await setSearch(query);
    const rows = document.querySelectorAll("li[data-translation-row]");
    expect(rows).toHaveLength(rowCount);
    // flat results are prefixed with flag + language
    expect(rows[0]?.querySelector("[data-row-language]")?.textContent).toBeTruthy();
    // the rail narrows to the matching languages
    expect(railOptions().length).toBeLessThanOrEqual(rowCount);
    // result count line reports the match total
    expect(document.querySelector("[data-results-count]")?.textContent).toContain(
      `${rowCount} translation`,
    );
  });

  it("reports no matches for an unmatched query and restores on clear", async () => {
    await open();
    await setSearch("zzz-no-match");
    expect(document.querySelector("li[data-translation-row]")).toBeNull();
    expect(railOptions()).toHaveLength(0);
    expect(document.querySelector('[role="status"]')?.textContent).toContain("No translations");
    await setSearch("");
    expect(railOptions()).toHaveLength(8);
    expect(paneRowIds().length).toBeGreaterThan(0);
  });

  it("searches by native-script autonym (stress S2: اردو and mixed-script urdu اردو)", async () => {
    await open();
    await setSearch("\u0627\u0631\u062F\u0648");
    // both Urdu rows via the autonym haystack, ordered name-first within the
    // language (flat cross-language sort: language, then name)
    expect(paneRowIds()).toEqual(["qul.ur.bayan", "ur.jalandhry"]);
    expect(railOptions()).toHaveLength(1);
    // mixed script narrows to the same rows: with the autonym in the haystack
    // both tokens hit the SAME rows, so every-token AND yields them
    await setSearch("urdu \u0627\u0631\u062F\u0648");
    expect(paneRowIds()).toEqual(["qul.ur.bayan", "ur.jalandhry"]);
  });

  it("toggles a stacked extra and syncs the ?more= url param", async () => {
    await open();
    railOptions().find((b) => b.getAttribute("data-language-option") === "Urdu")?.click();
    await settle();
    const box = checkboxes().find((c) => c.getAttribute("aria-label") === "Maulana Jalal ad-Din");
    expect(box).toBeTruthy();
    box?.click();
    expect([...stackedTranslations.ids]).toEqual(["ur.jalandhry"]);
    expect(h.replaceState).toHaveBeenCalledTimes(1);
    // SAFETY: replaceState is always called with a withMoreParam-built URL instance
    const url = h.replaceState.mock.calls[0]?.[0] as URL;
    expect(url.searchParams.get("more")).toBe("ur.jalandhry");
  });

  it("navigates to another translation preserving position and records the source", async () => {
    h.nav.url = new URL("https://example.test/app/t/ms/basmeih/juz/30");
    await open({ primaryId: "en.sahih" });
    const link = switchLinks().find((a) =>
      a.getAttribute("aria-label")?.includes("Saheeh International"),
    );
    expect(link?.getAttribute("href")).toContain("/app/t/en/sahih/juz/30");
    link?.click();
    expect(h.setSourceId).toHaveBeenCalledWith("en.sahih");
    await settle();
    // modal closed after the primary switch
    expect(document.querySelector("input[type='search']")).toBeNull();
  });

  it("disables unselected rows when the cap of five extras is reached", async () => {
    stackedTranslations.setIds(FILLER_IDS);
    await open({ primaryId: "en.sahih" });
    railOptions().find((b) => b.getAttribute("data-language-option") === "Indonesian")?.click();
    await settle();
    const extra = checkboxes().find((c) => c.getAttribute("aria-label") === "Kemenag");
    expect(extra?.disabled).toBe(true);
    // footer explains the cap, and swaps to the full note at capacity
    expect(document.querySelector("[data-cap-note]")?.textContent).toContain(
      "Remove one to add another",
    );
  });

  it("carries the quiet cap note and Done button in the footer; Done closes", async () => {
    await open();
    expect(document.querySelector("[data-cap-note]")?.textContent).toContain(
      "Up to 5 translations alongside the primary",
    );
    // SAFETY: selector matches only the footer Done button element
    const done = document.querySelector("button[data-done]") as HTMLButtonElement | null;
    expect(done?.textContent?.trim()).toBe("Done");
    done?.click();
    await settle();
    expect(document.querySelector("input[type='search']")).toBeNull();
  });
});

describe("TranslationModal — live reader position (stress S1)", () => {
  afterEach(() => {
    // Tests within a file share the happy-dom window; restore the default
    // non-reader location so the other describes stay on the fallback path.
    window.history.replaceState({}, "", "/");
  });

  it("derives the switch href from the live url on a scrolled surah route", async () => {
    // page.url still holds the bare surah slug (SvelteKit 2.70.2 replaceState
    // never updates the page store), while the reader's scroll handler has
    // rewritten window.location to /page/2 — the switch must carry the page.
    h.nav.url = new URL("https://example.test/app/al-baqarah");
    window.history.replaceState({}, "", "/app/al-baqarah/page/2");
    await open({ primaryId: "en.sahih" });
    const link = switchLinks().find((a) => a.getAttribute("aria-label")?.includes("Pickthall"));
    expect(link?.getAttribute("href")).toContain("/app/al-baqarah/t/en/pickthall/page/2");
  });

  it("keeps deriving from the page-store url when the live url is not a reader route", async () => {
    // window.location stays at the default "/": the page-store url carries
    // the position (the juz kind never gets scroll-rewritten).
    h.nav.url = new URL("https://example.test/app/juz/2");
    await open({ primaryId: "en.sahih" });
    const link = switchLinks().find((a) => a.getAttribute("aria-label")?.includes("Pickthall"));
    expect(link?.getAttribute("href")).toContain("/app/t/en/pickthall/juz/2");
  });

  it("re-derives the live position on every reopen, not just the first", async () => {
    h.nav.url = new URL("https://example.test/app/al-baqarah");
    window.history.replaceState({}, "", "/app/al-baqarah/page/2");
    // Default no-op: TS cannot see the expose callback has run, so a null
    // initializer would narrow the later calls to `never`.
    let setOpen: (open: boolean) => void = () => {};
    instance = mount(TranslationModalHost, {
      target,
      props: {
        primaryId: "en.sahih",
        expose: (setter: (open: boolean) => void) => {
          setOpen = setter;
        },
      },
    });
    await settle();
    const pickthallHref = (): string | null | undefined =>
      switchLinks()
        .find((a) => a.getAttribute("aria-label")?.includes("Pickthall"))
        ?.getAttribute("href");
    expect(pickthallHref()).toContain("/app/al-baqarah/t/en/pickthall/page/2");
    // Close the SAME mounted instance (as the header button binding does),
    // let the reader "scroll" (rewrite the live url), then reopen: the cached
    // position from the previous open must not survive.
    setOpen(false);
    await settle();
    window.history.replaceState({}, "", "/app/al-baqarah/page/4");
    setOpen(true);
    await settle();
    expect(pickthallHref()).toContain("/app/al-baqarah/t/en/pickthall/page/4");
  });
});

describe("TranslationModal — selected chips", () => {
  it("pins chips in current order, primary first with a badged tooltip trigger", async () => {
    stackedTranslations.setIds(["ur.jalandhry", "ms.basmeih"]);
    await open({ primaryId: "en.sahih" });
    const chips = [...document.querySelectorAll("[data-selected-chips] [data-chip]")];
    expect(chips.map((c) => c.getAttribute("data-chip"))).toEqual([
      "en.sahih",
      "ur.jalandhry",
      "ms.basmeih",
    ]);
    const primaryChip = chips[0];
    expect(primaryChip?.textContent).toContain("Primary");
    // the badge is a tooltip trigger explaining the primary's role
    expect(primaryChip?.querySelector('button[aria-label]')?.getAttribute("aria-label")).toContain(
      "reading mode",
    );
    // extras carry hover-revealed reorder arrows + remove
    const extraChip = chips[1];
    expect(extraChip?.querySelector('button[aria-label="Move up"]')).toBeTruthy();
    expect(extraChip?.querySelector('button[aria-label="Move down"]')).toBeTruthy();
    expect(extraChip?.querySelector('button[aria-label="Remove"]')).toBeTruthy();
    // right-aligned meta: cap indicator + clear-all
    const meta = document.querySelector("[data-selected-chips]");
    expect(meta?.textContent).toContain("2/5");
    expect(meta?.textContent).toContain("Clear all");
  });

  it("reorders, removes, and clears extras from the chips row, syncing ?more=", async () => {
    stackedTranslations.setIds(["ur.jalandhry", "ms.basmeih"]);
    await open({ primaryId: "en.sahih" });
    const chip = document.querySelector('[data-chip="ms.basmeih"]');
    chip?.querySelector('button[aria-label="Move up"]')?.dispatchEvent(
      new MouseEvent("click", { bubbles: true }),
    );
    expect([...stackedTranslations.ids]).toEqual(["ms.basmeih", "ur.jalandhry"]);
    document
      .querySelector('[data-chip="ur.jalandhry"] button[aria-label="Remove"]')
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect([...stackedTranslations.ids]).toEqual(["ms.basmeih"]);
    const clearAll = [...document.querySelectorAll("[data-selected-chips] button")].find((b) =>
      b.textContent?.trim() === "Clear all",
    );
    clearAll?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect([...stackedTranslations.ids]).toEqual([]);
    // SAFETY: every replaceState call receives a withMoreParam-built URL instance
    const lastUrl = h.replaceState.mock.calls.at(-1)?.[0] as URL;
    expect(lastUrl.searchParams.get("more")).toBeNull();
  });

  it("updates the chips row live when toggling a pane row", async () => {
    await open();
    expect(document.querySelector("[data-selected-chips]")).toBeNull();
    railOptions().find((b) => b.getAttribute("data-language-option") === "Urdu")?.click();
    await settle();
    const box = checkboxes().find((c) => c.getAttribute("aria-label") === "Maulana Jalal ad-Din");
    box?.click();
    await settle();
    expect(document.querySelector("[data-selected-chips]")?.textContent).toContain("Jalandhry");
    box?.click();
    await settle();
    expect(document.querySelector("[data-selected-chips]")).toBeNull();
  });
});

describe("TranslationModal — touch & tap targets (stress S8-S14)", () => {
  it("makes the whole row the toggle target via a label cover over checkbox+name+author", async () => {
    await open({ primaryId: "en.sahih" });
    const row = pane().querySelector('li[data-translation-row="en.pickthall"]');
    // SAFETY: selector matches only the row's label cover element
    const label = row?.querySelector("label[data-row-target]") as HTMLLabelElement | null;
    const box = row?.querySelector('input[type="checkbox"]');
    expect(label).toBeTruthy();
    // the label is associated with the row checkbox, so its whole surface toggles
    expect(label?.getAttribute("for")).toBe(box?.id);
    expect(box?.id).toBe("tmodal-en.pickthall");
    // >=44px cover + no double-tap-zoom window on it
    expect(label?.className).toContain("min-h-11");
    expect(label?.className).toContain("touch-manipulation");
    // the name link stretches to the full row height (>=44px target) and keeps
    // the primary-switch role
    const link = row?.querySelector("a[data-switch]");
    expect(link?.className).toContain("self-stretch");
    // clicking the row cover (not the checkbox) toggles the selection
    label?.click();
    await settle();
    expect([...stackedTranslations.ids]).toEqual(["en.pickthall"]);
    // primary rows keep a plain cover: no checkbox to associate, no dead label
    const primaryRow = pane().querySelector('li[data-translation-row="en.sahih"]');
    expect(primaryRow?.querySelector("label[data-row-target]")).toBeNull();
    expect(primaryRow?.querySelector("div[data-row-cover]")).toBeTruthy();
  });

  it("keeps chip reorder arrows focusable and revealed without hover", async () => {
    stackedTranslations.setIds(["ur.jalandhry", "ms.basmeih"]);
    await open({ primaryId: "en.sahih" });
    // SAFETY: selector matches only the named chip control button
    const up = document.querySelector(
      '[data-chip="ms.basmeih"] button[aria-label="Move up"]',
    ) as HTMLButtonElement | null;
    // SAFETY: selector matches only the named chip control button
    const down = document.querySelector(
      '[data-chip="ur.jalandhry"] button[aria-label="Move down"]',
    ) as HTMLButtonElement | null;
    // real focusable buttons with labels: keyboard users can reach them
    expect(up?.tagName).toBe("BUTTON");
    expect(up?.tabIndex).toBe(0);
    expect(down?.tabIndex).toBe(0);
    // class-level visibility contract (jsdom cannot match media queries): the
    // hover-reveal variant compiles only under @media(hover:hover), so coarse
    // pointers need the hover:none reveal and keyboard needs focus-within.
    expect(up?.className).toContain("[@media(hover:none)]:enabled:opacity-100");
    expect(up?.className).toContain("group-focus-within/chip:opacity-100");
    expect(down?.className).toContain("[@media(hover:none)]:enabled:opacity-100");
    expect(down?.className).toContain("group-focus-within/chip:opacity-100");
    // disabled end arrows stay dimmed and unfocusable, not silent dead zones
    // SAFETY: selector matches only the named chip control button
    const disabledUp = document.querySelector(
      '[data-chip="ur.jalandhry"] button[aria-label="Move up"]',
    ) as HTMLButtonElement | null;
    expect(disabledUp?.disabled).toBe(true);
    expect(disabledUp?.className).toContain("disabled:opacity-30");
  });

  it("lays the chips out as a single horizontal scroll row, never a wall", async () => {
    stackedTranslations.setIds(["ur.jalandhry", "ms.basmeih"]);
    await open({ primaryId: "en.sahih" });
    // SAFETY: the chips strip is the first div child of the chips row
    const strip = document.querySelector("[data-selected-chips] > div") as HTMLElement | null;
    expect(strip?.className).toContain("overflow-x-auto");
    expect(strip?.className).not.toContain("flex-wrap");
    expect(strip?.className).toContain("overscroll-contain");
    const chips = [...document.querySelectorAll("[data-selected-chips] [data-chip]")];
    expect(chips).toHaveLength(3);
    for (const chip of chips) {
      expect(chip.className).toContain("flex-none");
      expect(chip.className).toContain("h-11");
    }
    // chip controls are 44px tall with a pseudo-widened hit area
    const remove = document.querySelector('[data-chip="ms.basmeih"] button[aria-label="Remove"]');
    expect(remove?.className).toContain("h-11");
    expect(remove?.className).toContain("before:-inset-x-2");
  });

  it("shows the provenance label as an inline muted line on coarse pointers only", async () => {
    await open({ primaryId: "en.sahih" });
    const source = pane().querySelector(
      'li[data-translation-row="en.pickthall"] [data-row-source]',
    );
    expect(source?.textContent).toContain("Tanzil");
    // hover-capable devices keep the clean row (tooltip carries provenance)
    expect(source?.className).toContain("[@media(hover:hover)]:hidden");
  });

  it("contains overscroll in the scroll containers and pads the dialog for safe areas", async () => {
    await open();
    expect(rail().className).toContain("overscroll-contain");
    expect(pane().className).toContain("overscroll-contain");
    // SAFETY: bits-ui marks the portaled dialog content element
    const content = document.querySelector("[data-dialog-content]");
    expect(content?.className).toContain("env(safe-area-inset-left)");
    expect(content?.className).toContain("env(safe-area-inset-right)");
  });

  it("gives Clear all, Done, Close, and the search clear ≥44px effective targets", async () => {
    stackedTranslations.setIds(["ur.jalandhry"]);
    await open({ primaryId: "en.sahih" });
    const clearAll = [...document.querySelectorAll("[data-selected-chips] button")].find((b) =>
      b.textContent?.trim() === "Clear all",
    );
    // transparent button: real vertical padding + before-pseudo extension
    expect(clearAll?.className).toContain("py-2.5");
    expect(clearAll?.className).toContain("before:-inset-2");
    const done = document.querySelector("button[data-done]");
    expect(done?.className).toContain("h-11");
    // SAFETY: selector matches only the header Close button element
    const close = document.querySelector('button[aria-label="Close"]') as HTMLElement | null;
    expect(close?.className).toContain("h-11");
    expect(close?.className).toContain("w-11");
    await setSearch("urdu");
    // SAFETY: selector matches only the search field's clear button element
    const searchClear = document.querySelector(
      'button[aria-label="Clear search"]',
    ) as HTMLElement | null;
    expect(searchClear?.className).toContain("before:-inset-2");
    const back = pane().querySelector('button[aria-label="Back"]');
    expect(back?.className).toContain("h-11");
  });
});

describe("TranslationModal — reading mode constraint", () => {
  it("disables every checkbox and shows the notice, keeping primary navigation", async () => {
    h.readerStub.isVerseMode = false;
    h.readerStub.isReadingMode = true;
    stackedTranslations.setIds(["ur.jalandhry"]);
    await open({ primaryId: "en.sahih" });
    expect(checkboxes().length).toBeGreaterThan(0);
    for (const box of checkboxes()) {
      expect(box.disabled).toBe(true);
    }
    expect(pane().textContent).toContain("Only one translation is shown in reading mode");
    // no chips row in reading mode (selection is constrained to one)
    expect(document.querySelector("[data-selected-chips]")).toBeNull();
    // primary navigation stays available
    expect(switchLinks().length).toBeGreaterThan(0);
  });
});

describe("TranslationModal — mobile collapse", () => {
  it("hides the rail once a language is tapped and returns via Back", async () => {
    await open();
    expect(rail().className).not.toContain("hidden");
    railOptions().find((b) => b.getAttribute("data-language-option") === "Urdu")?.click();
    await settle();
    expect(rail().className).toContain("hidden");
    expect(pane().className).toContain("flex");
    const back = pane().querySelector('button[aria-label="Back"]');
    expect(back).toBeTruthy();
    back?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await settle();
    expect(rail().className).not.toContain("hidden");
  });

  it("shows the pane over the rail while searching on small screens", async () => {
    await open();
    await setSearch("urdu");
    expect(rail().className).toContain("hidden");
    expect(pane().className).toContain("flex");
  });
});

describe("TranslationButton badge", () => {
  it("shows the stacked count excluding the route primary", () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry", "ms.basmeih"]);
    instance = mount(TranslationButton, { target, props: { primaryId: "en.sahih" } });
    const badge = target.querySelector("button[aria-haspopup='dialog'] span");
    expect(badge?.textContent?.trim()).toBe("2");
  });

  it("explains hidden translations on the badge tooltip in reading mode", () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    h.readerStub.isVerseMode = false;
    h.readerStub.isReadingMode = true;
    instance = mount(TranslationButton, { target, props: { primaryId: "en.sahih" } });
    const badge = target.querySelector("button[aria-haspopup='dialog'] span");
    expect(badge?.getAttribute("title")).toBe(
      "Reading mode shows only your first translation.",
    );
  });

  it("opens the modal on click", async () => {
    instance = mount(TranslationButton, { target, props: { primaryId: "en.sahih" } });
    // SAFETY: selector matches only the trigger button element
    const button = target.querySelector("button[aria-haspopup='dialog']") as HTMLButtonElement;
    expect(document.querySelector("input[type='search']")).toBeNull();
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, composed: true }));
    await settle();
    // the open state drives the portaled modal (its open rendering is covered by
    // the TranslationModal tests above); here we pin the trigger contract.
    expect(button.getAttribute("aria-expanded")).toBe("true");
  });
});
