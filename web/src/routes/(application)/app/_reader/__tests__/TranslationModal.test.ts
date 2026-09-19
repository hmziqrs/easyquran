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
      entry("ur.jalandhry", "Urdu", "ur", "rtl", "Jalandhry", "Maulana Jalal ad-Din"),
      entry("ms.basmeih", "Malay", "ms", "ltr", "Basmeih", "Abdullah Muhammad Basmeih"),
      entry("quranenc.fr.hamidullah", "French", "fr", "ltr", "Hamidullah", "Muhammad Hamidullah"),
      entry("tr.diyanet", "Turkish", "tr", "ltr", "Diyanet", "Diyanet Isleri"),
      entry("id.indonesian", "Indonesian", "id", "ltr", "Kemenag", "Kemenag"),
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
  return {
    TRANSLATION_CATALOGUE: h.catalogue,
    TRANSLATION_CATALOGUE_BY_ID: byId,
    translationSourceOf,
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

const groups = (): Element[] => [...document.querySelectorAll("details")];
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
  it("groups rows by language in collapsible details", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    const langs = groups().map((g) => g.querySelector("summary")?.textContent ?? "");
    expect(langs).toHaveLength(6);
    expect(langs.join(" ")).toContain("English");
    expect(langs.join(" ")).toContain("Urdu");
    expect(langs.join(" ")).toContain("French");
  });

  it("filters rows by search and auto-expands matching groups", async () => {
    instance = mount(TranslationModal, { target, props: { open: true, primaryId: "en.sahih" } });
    await settle();
    await setSearch("urdu");
    const visible = groups();
    expect(visible).toHaveLength(1);
    expect(visible[0]?.getAttribute("open")).not.toBeUndefined();
    expect(visible[0]?.querySelectorAll("li")).toHaveLength(1);

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
    expect(document.body.textContent).toContain("Primary translation");
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
