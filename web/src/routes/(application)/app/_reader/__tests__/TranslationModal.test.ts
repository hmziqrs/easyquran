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
    ["fr", { flag: "\u{1F1EB}\u{1F1F7}", country: "France" }],
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

beforeEach(() => {
  h.readerStub.isVerseMode = true;
  h.readerStub.isReadingMode = false;
  h.nav.url = new URL("https://example.test/app/al-fatihah");
  h.replaceState.mockClear();
  h.setSourceId.mockClear();
  localStorage.clear();
  stackedTranslations.clear();
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
  // bits-ui portals can leave dialog shells behind across mounts; clear them so
  // document-level queries only ever see the current modal under test.
  for (const el of document.querySelectorAll("[data-dialog-content], [data-dialog-overlay]")) {
    el.remove();
  }
});

const groups = (): Element[] => [...document.querySelectorAll("section[data-language]")];
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

describe("TranslationModal", () => {
  it("renders static groups sorted alphabetically by language, no collapsibles", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    expect(document.querySelectorAll("details")).toHaveLength(0);
    expect(document.querySelectorAll("summary")).toHaveLength(0);
    const langs = groups().map((g) => g.getAttribute("data-language") ?? "");
    expect(langs).toEqual(["English", "French", "Indonesian", "Malay", "Spanish", "Turkish", "Urdu"]);
    // every row of every group is present without interacting with headers
    expect(document.querySelectorAll("section[data-language] li")).toHaveLength(11);
  });

  it("shows a flag emoji next to each language group header", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const english = groups().find((g) => g.getAttribute("data-language") === "English");
    expect(english?.querySelector("div")?.textContent).toContain("\u{1F1EC}\u{1F1E7}");
  });

  it("sorts rows within a group by translation name", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const english = groups().find((g) => g.getAttribute("data-language") === "English");
    const names = [...(english?.querySelectorAll("input[type='checkbox']") ?? [])].map((c) =>
      c.getAttribute("aria-label"),
    );
    expect(names).toEqual(["Ahmed Ali", "Marmaduke Pickthall", "Saheeh International", "Wahiduddin Khan"]);
  });

  it("focus ring sits on the search wrapper, not the inner input", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const input = searchInput();
    const wrapper = input.closest("div");
    expect(wrapper?.className).toContain("focus-within:border-border-strong");
    expect(input.className).not.toContain("focus-visible:outline-2");
  });

  it.each([
    ["urdu", "Urdu", 2],
    ["englsh", "English", 4],
    ["israr", "Urdu", 1],
    ["khan", "English", 1],
    ["turkey", "Turkish", 1],
    ["cortes", "Spanish", 1],
  ])("fuzzy search %s finds the %s group (%i rows)", async (query, language, rowCount) => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    await setSearch(query);
    const matching = groups().filter((g) => g.getAttribute("data-language") === language);
    expect(matching).toHaveLength(1);
    expect(matching[0]?.querySelectorAll("li")).toHaveLength(rowCount);
  });

  it("reports no matches for an unmatched query", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    await setSearch("zzz-no-match");
    expect(groups()).toHaveLength(0);
    expect(document.querySelector('[role="status"]')?.textContent).toContain("No translations");
  });

  it("toggles a stacked extra and syncs the ?more= url param", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
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

  it("disables the primary translation's checkbox with a primary badge", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const primary = checkboxes().find((c) => c.getAttribute("aria-label") === "Saheeh International");
    expect(primary?.disabled).toBe(true);
    expect(document.body.textContent).toContain("Primary");
  });

  it("caps stacked extras at five — unselected rows are disabled when full", async () => {
    stackedTranslations.setIds(FILLER_IDS);
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const extra = checkboxes().find((c) => c.getAttribute("aria-label") === "Kemenag");
    expect(extra?.disabled).toBe(true);
    expect(document.body.textContent).toContain("remove one to add another");
  });

  it("hides checkboxes in reading mode and shows the single-translation notice", async () => {
    h.readerStub.isVerseMode = false;
    h.readerStub.isReadingMode = true;
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    expect(checkboxes()).toHaveLength(0);
    expect(document.body.textContent).toContain("Only one translation is shown in reading mode");
    // primary navigation stays available
    expect(switchLinks().length).toBeGreaterThan(0);
  });

  it("navigates to another translation preserving position and records the source", async () => {
    h.nav.url = new URL("https://example.test/app/t/ms/basmeih/juz/30");
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: null } });
    await settle();
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

  it("pins the Selected section above the groups: primary first (badge), extras in order", async () => {
    stackedTranslations.setIds(["ur.jalandhry", "ms.basmeih"]);
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const selected = document.querySelector("[data-selected-section]");
    expect(selected).toBeTruthy();
    const firstGroup = groups()[0];
    // SAFETY: selected is asserted truthy above; this cast is a non-null DOM node
    const selectedEl = selected as Element;
    // SAFETY: firstGroup is the first [data-language] section, always present when groups render
    const groupEl = firstGroup as Element;
    expect(
      selectedEl.compareDocumentPosition(groupEl) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const names = [...(selected?.querySelectorAll("li .truncate.font-medium") ?? [])].map((el) =>
      el.textContent?.trim(),
    );
    expect(names).toEqual(["Saheeh International", "Jalandhry", "Basmeih"]);
    // primary is visually distinct via the Primary badge
    expect(selected?.textContent).toContain("Primary");
  });

  it("updates the Selected section live when toggling from the grouped list", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: null } });
    await settle();
    expect(document.querySelector("[data-selected-section]")).toBeNull();
    const box = checkboxes().find((c) => c.getAttribute("aria-label") === "Maulana Jalal ad-Din");
    // clickable row label carries the pointer cursor
    expect(box?.closest("li")?.querySelector("label")?.className).toContain("cursor-pointer");
    box?.click();
    await settle();
    const selected = document.querySelector("[data-selected-section]");
    expect(selected?.textContent).toContain("Jalandhry");
    box?.click();
    await settle();
    expect(document.querySelector("[data-selected-section]")).toBeNull();
  });

  it("omits the Selected section entirely when nothing is selected", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: null } });
    await settle();
    expect(document.querySelector("[data-selected-section]")).toBeNull();
    expect(document.body.textContent).not.toContain("No translations selected");
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
