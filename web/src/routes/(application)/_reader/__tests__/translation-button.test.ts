import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

// The badge tests mount the real TranslationButton, whose graph reaches the
// shared TranslationModal (now #lib/components/translation) — the same mock
// wall TranslationModal.test.ts builds keeps this hermetic: catalogue, region
// detection, hotkeys, kit state and the storage-report env stubs.
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
      entry("ur.jalandhry", "Urdu", "ur", "rtl", "Jalandhry", "Maulana Jalal ad-Din"),
      entry("ms.basmeih", "Malay", "ms", "ltr", "Basmeih", "Abdullah Muhammad Basmeih"),
    ],
    nav: {
      url: new URL("https://example.test/al-fatihah"),
      // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
      params: {} as Record<string, string>,
      state: {},
    },
    goto: vi.fn().mockResolvedValue(undefined),
    readerStub: {
      isVerseMode: true,
      isReadingMode: false,
    },
  };
});

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  PUBLIC_TURNSTILE_SITE_KEY: undefined,
}));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$app/navigation", () => ({ goto: h.goto }));
vi.mock("#lib/stores/reader.svelte.js", () => ({ reader: h.readerStub }));
vi.mock("#lib/hotkeys.svelte.js", () => ({
  registerHotkey: () => ({ unregister: () => {} }),
}));
vi.mock("#lib/components/translation/translation-region.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("#lib/components/translation/translation-region.js")>();
  return { ...actual, detectCountries: () => [] };
});
vi.mock("#lib/quran/catalogue.js", () => {
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
    flagFor: () => ({ flag: "\u{1F310}", country: "" }),
    nativeNameFor: () => null,
  };
});

import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";

import TranslationButton from "../TranslationButton.svelte";

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  h.readerStub.isVerseMode = true;
  h.readerStub.isReadingMode = false;
  h.goto.mockClear();
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

// bits-ui portals dialog content in after mount (presence transition) — let a
// macrotask tick land before querying the portaled DOM.
function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

describe("TranslationButton badge", () => {
  it("shows the stacked count excluding the route translation", () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry", "ms.basmeih"]);
    instance = mount(TranslationButton, { target, props: { routeTranslationId: "en.sahih" } });
    const badge = target.querySelector("[data-translation-count]");
    expect(badge?.textContent?.trim()).toBe("2");
  });

  it("carries no reading-mode warning on the badge (reading picks its text in the header)", () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    h.readerStub.isVerseMode = false;
    h.readerStub.isReadingMode = true;
    instance = mount(TranslationButton, { target, props: { routeTranslationId: "en.sahih" } });
    const badge = target.querySelector("[data-translation-count]");
    expect(badge?.getAttribute("title")).toBeNull();
  });

  it("opens the modal on click", async () => {
    instance = mount(TranslationButton, { target, props: { routeTranslationId: "en.sahih" } });
    // SAFETY: selector matches only the trigger button element
    const button = target.querySelector("button[aria-haspopup='dialog']") as HTMLButtonElement;
    expect(document.querySelector("input[type='search']")).toBeNull();
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, composed: true }));
    await settle();
    // the open state drives the portaled modal (its open rendering is covered by
    // TranslationModal.test.ts); here we pin the trigger contract.
    expect(button.getAttribute("aria-expanded")).toBe("true");
  });
});
