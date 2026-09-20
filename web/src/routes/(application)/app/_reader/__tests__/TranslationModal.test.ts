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
  return { ...actual, deLocalizeUrl: (url: URL) => url };
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
    ["en", { flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" }],
    ["ur", { flag: "\u{1F1F5}\u{1F1F0}", country: "Pakistan" }],
    ["ms", { flag: "\u{1F1F2}\u{1F1FE}", country: "Malaysia" }],
    ["fr", { flag: "\u{1F1EB}\u{1F1E7}", country: "France" }],
    ["tr", { flag: "\u{1F1F9}\u{1F1F7}", country: "Turkey" }],
    ["id", { flag: "\u{1F1EE}\u{1F1E9}", country: "Indonesia" }],
    ["es", { flag: "\u{1F1EA}\u{1F1F8}", country: "Spain" }],
  ]);
  return {
    TRANSLATION_CATALOGUE: h.catalogue,
    TRANSLATION_CATALOGUE_BY_ID: byId,
    translationSourceOf,
    flagFor: (code: string) =>
      flags.get(code) ?? { flag: "\u{1F310}", country: "" },
  };
});

import { stackedTranslations } from "$lib/stores/stacked-translations.svelte";
import TranslationButton from "../TranslationButton.svelte";
import TranslationModal from "../TranslationModal.svelte";

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
  it("renders the language rail alphabetically with flag and count per language", async () => {
    await open();
    // each rail row is three spans: flag, language name, count
    const names = railOptions().map((b) => b.querySelectorAll("span")[1]?.textContent ?? "");
    expect(names).toEqual([
      "English",
      "French",
      "Indonesian",
      "Malay",
      "Spanish",
      "Turkish",
      "Urdu",
    ]);
    const english = railOptions()[0];
    expect(english?.textContent).toContain("\u{1F1EC}\u{1F1E7}");
    expect(english?.textContent).toContain("4");
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

    const english = railOptions()[0];
    english?.focus();
    english?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    );
    await settle();
    const scrolledOnKey = scrolledIntoView.map((el) => el.getAttribute("data-language-option"));
    expect(scrolledOnKey).toContain("French");
  });

  it("pluralizes the count line: 4 translations vs 1 translation", async () => {
    await open();
    expect(document.querySelector("[data-results-count]")?.textContent?.trim()).toBe(
      "4 translations",
    );
    railOptions().find((b) => b.getAttribute("data-language-option") === "Turkish")?.click();
    await settle();
    expect(document.querySelector("[data-results-count]")?.textContent?.trim()).toBe(
      "1 translation",
    );
  });

  it("selecting a rail language swaps the pane rows", async () => {
    await open();
    railOptions().find((b) => b.getAttribute("data-language-option") === "Turkish")?.click();
    await settle();
    expect(paneRowIds()).toEqual(["tr.diyanet"]);
    expect(pane().querySelector("h3")?.textContent).toContain("Turkish");
  });

  it("moves keyboard focus through the rail with arrow keys", async () => {
    await open({ primaryId: "en.sahih" });
    const english = railOptions()[0];
    english?.focus();
    expect(document.activeElement).toBe(english);
    english?.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    );
    await settle();
    const french = railOptions()[1];
    expect(document.activeElement).toBe(french);
    expect(french?.getAttribute("aria-current")).toBe("true");
    french?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    await settle();
    expect(document.activeElement).toBe(english);
    expect(english?.getAttribute("aria-current")).toBe("true");
  });

  it("uses one row anatomy: name first, author line only when it differs", async () => {
    await open({ primaryId: null });
    const pickthall = pane().querySelector('li[data-translation-row="en.pickthall"]');
    expect(pickthall?.textContent).toContain("Pickthall");
    expect(pickthall?.querySelector("[data-author-line]")?.textContent).toContain(
      "Marmaduke Pickthall",
    );
    // translator mirrors the name: no duplicated second line
    const sahih = pane().querySelector('li[data-translation-row="en.sahih"]');
    expect(sahih?.querySelector("[data-author-line]")).toBeNull();
    // every row carries an 18px checkbox and a semantic source dot
    expect(pickthall?.querySelector('input[type="checkbox"]')?.className).toContain("size-[18px]");
    expect(pickthall?.querySelector("button span.size-2")?.className).toContain(
      "bg-emerald-500",
    );
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
    expect(railOptions()).toHaveLength(7);
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
    expect(railOptions()).toHaveLength(7);
    expect(paneRowIds().length).toBeGreaterThan(0);
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
    await open();
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
