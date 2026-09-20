import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => {
  const entry = (
    id: string,
    language: string,
    languageCode: string,
    translator: string,
    direction: "ltr" | "rtl" = "ltr",
  ) => ({
    id,
    language,
    languageCode,
    direction,
    name: `Name ${id}`,
    translator,
    sizeBytes: 2048,
    downloadUrl: "",
  });
  return {
    catalogue: [
      entry("en.sahih", "English", "en", "Saheeh International"),
      entry("en.arberry", "English", "en", "Arthur J. Arberry"),
      entry("ur.jalandhry", "Urdu", "ur", "Maulana Jalal ad-Din", "rtl"),
    ],
    nav: {
      // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests only ever assign string entries
      params: {} as Record<string, string>,
      state: {},
    },
  };
});

vi.mock("$app/environment", () => ({ browser: true }));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$lib/quran/catalogue", () => ({
  flagFor: (code: string) =>
    code === "ur"
      ? { flag: "\u{1F1F5}\u{1F1F0}", country: "Pakistan" }
      : { flag: "\u{1F1EC}\u{1F1E7}", country: "United Kingdom" },
  translationSourceOf: (id: string): string => {
    if (id.startsWith("qul.")) return "qul";
    if (id.startsWith("quranenc.")) return "quranenc";
    return "tanzil";
  },
}));

import type { ReadingCandidate } from "../reading-mode-guard.svelte";
import ReadingModeDialog from "../ReadingModeDialog.svelte";

function candidate(id: string | null): ReadingCandidate {
  if (id === null) return { id: null, entry: null };
  const entry = h.catalogue.find((t) => t.id === id);
  if (!entry) throw new Error(`unknown fixture id ${id}`);
  return { id, entry };
}

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;
let confirmed: ReadingCandidate[] = [];

beforeEach(() => {
  confirmed = [];
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

function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

async function openDialog(candidates: ReadingCandidate[]) {
  instance = mount(ReadingModeDialog, {
    target,
    props: { open: true, candidates, onConfirm: (c: ReadingCandidate) => confirmed.push(c) },
  });
  await settle();
}

const radios = (): HTMLInputElement[] => {
  // SAFETY: selector matches only radio inputs, so every element is HTMLInputElement
  return [...document.querySelectorAll('input[type="radio"]')] as HTMLInputElement[];
};
const confirmButton = (): HTMLButtonElement => {
  // SAFETY: selector matches only the confirm button element
  return document.querySelector("button[data-reading-confirm]") as HTMLButtonElement;
};

describe("ReadingModeDialog", () => {
  it("lists the current source first, then extras, with the current source preselected", async () => {
    await openDialog([
      candidate("en.sahih"),
      candidate("en.arberry"),
      candidate("ur.jalandhry"),
    ]);
    const inputs = radios();
    expect(inputs.map((r) => r.value)).toEqual(["en.sahih", "en.arberry", "ur.jalandhry"]);
    expect(inputs[0]?.checked).toBe(true);
    expect(inputs[1]?.checked).toBe(false);
    // translator-first labels + language sublabels
    expect(document.body.textContent).toContain("Saheeh International");
    expect(document.body.textContent).toContain("Maulana Jalal ad-Din");
    expect(document.body.textContent).toContain("Urdu");
  });

  it("confirms the selected candidate (choice differs from the preselection)", async () => {
    await openDialog([candidate("en.sahih"), candidate("en.arberry")]);
    const second = radios()[1];
    second?.click();
    confirmButton().click();
    expect(confirmed.map((c) => c.id)).toEqual(["en.arberry"]);
  });

  it("cancel leaves nothing confirmed", async () => {
    await openDialog([candidate("en.sahih"), candidate("en.arberry")]);
    const cancel = [...document.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Cancel"),
    );
    cancel?.click();
    await settle();
    expect(confirmed).toEqual([]);
  });

  it("renders the simplified single-candidate confirm without a radio list", async () => {
    await openDialog([candidate("en.sahih")]);
    expect(radios()).toHaveLength(0);
    expect(document.body.textContent).toContain("Saheeh International");
    confirmButton().click();
    expect(confirmed.map((c) => c.id)).toEqual(["en.sahih"]);
  });

  it("labels the Arabic current source with the Arabic copy and no provenance chip", async () => {
    await openDialog([candidate(null), candidate("en.arberry")]);
    const first = radios()[0];
    expect(first?.value).toBe("arabic");
    expect(first?.checked).toBe(true);
    expect(document.body.textContent).toContain("Arabic");
    // only the translation candidate carries a provenance chip
    expect(document.querySelectorAll("span.bg-emerald-500, span.size-1\\.5")).toHaveLength(1);
  });
});
