import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => ({
  nav: {
    url: new URL("https://example.test/juz/15?mode=verse"),
    state: {},
  },
  replaceState: vi.fn(),
  reader: {
    // SAFETY: widens the literal to the store's ReaderMode union; setMode only assigns those two.
    mode: "verse" as "verse" | "reading",
    setMode(mode: "verse" | "reading") {
      h.reader.mode = mode;
    },
  },
}));

vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("$app/navigation", () => ({ replaceState: h.replaceState }));
vi.mock("#lib/stores/reader.svelte.js", () => ({ reader: h.reader }));

import ReaderModeToggle from "../ReaderModeToggle.svelte";
import { registerTypographyWrapper } from "../typography-change";

let target: HTMLElement;
let instance: ReturnType<typeof mount> | null = null;

beforeEach(() => {
  h.reader.mode = "verse";
  h.replaceState.mockClear();
  target = document.createElement("div");
  document.body.appendChild(target);
});

afterEach(() => {
  if (instance) void unmount(instance);
  instance = null;
  target.remove();
});

const option = (mode: string): HTMLButtonElement | null =>
  target.querySelector<HTMLButtonElement>(`[data-mode-option="${mode}"]`);

describe("ReaderModeToggle", () => {
  it("names both modes and marks the active one", () => {
    instance = mount(ReaderModeToggle, { target });
    expect(option("verse")?.getAttribute("aria-label")).toBe("Ayah-by-Ayah");
    expect(option("reading")?.getAttribute("aria-label")).toBe("Reading");
    expect(option("verse")?.getAttribute("aria-pressed")).toBe("true");
    expect(option("reading")?.getAttribute("aria-pressed")).toBe("false");
  });

  it("switches mode through the reader's anchor wrapper and writes ?mode=", () => {
    const wrapped = vi.fn((change: () => void) => change());
    const unregister = registerTypographyWrapper(wrapped);
    try {
      instance = mount(ReaderModeToggle, { target });
      option("reading")?.click();
      expect(wrapped).toHaveBeenCalledTimes(1);
      expect(h.reader.mode).toBe("reading");
      expect(h.replaceState).toHaveBeenCalledTimes(1);
      // SAFETY: replaceState is always called with a withModeParam-built URL instance
      const url = h.replaceState.mock.calls[0]?.[0] as URL;
      expect(url.searchParams.get("mode")).toBe("reading");
      // tapping the active mode is a no-op: no reflow, no URL write
      option("reading")?.click();
      expect(wrapped).toHaveBeenCalledTimes(1);
    } finally {
      unregister();
    }
  });
});
