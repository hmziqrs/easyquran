import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const trackMock = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("#lib/firebase/analytics.js", () => ({ track: trackMock }));
vi.mock("$app/env", () => ({ browser: true }));

// Each test re-imports the whole graph (svelte runtime included, so the component
// and mount() share one runtime): the component binds the localeSuggest singleton,
// and the shown-latch can only be reset by re-instantiating the world.
async function freshHarness() {
  vi.resetModules();
  trackMock.mockClear();
  const [svelte, suggest, auth, component] = await Promise.all([
    import("svelte"),
    import("#lib/i18n/locale-suggest.svelte.js"),
    import("#lib/auth/auth-modal.svelte.js"),
    import("#lib/components/i18n/LocaleSuggest.svelte"),
  ]);
  const target = document.createElement("div");
  document.body.appendChild(target);
  const instance = svelte.mount(component.default, { target });
  return {
    localeSuggest: suggest.localeSuggest,
    authModal: auth.authModal,
    target,
    dispose: () => {
      void svelte.unmount(instance);
    },
  };
}

function dialog(): HTMLElement | null {
  return document.querySelector('[data-slot="locale-suggest-dialog"]');
}

function findButton(label: string): HTMLButtonElement | null {
  for (const button of Array.from(document.querySelectorAll("button"))) {
    if ((button.getAttribute("aria-label") ?? button.textContent?.trim()) === label) {
      return button;
    }
  }
  return null;
}

function acceptLink(): HTMLAnchorElement | null {
  return document.querySelector("a[data-sveltekit-reload]");
}

function stored(): Record<string, string> {
  const parsed: { resolved?: Record<string, string> } = JSON.parse(
    localStorage.getItem("easyquran.localeSuggest") ?? "null",
  );
  return parsed.resolved ?? {};
}

async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) await Promise.resolve();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

let target: HTMLElement | null = null;
let dispose: (() => void) | null = null;

function stubArabicBrowser(): void {
  vi.stubGlobal("navigator", { languages: ["ar", "en"] });
}

beforeEach(() => localStorage.clear());
afterEach(() => {
  dispose?.();
  target?.remove();
  target = null;
  document.body.innerHTML = "";
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  localStorage.clear();
});

describe("LocaleSuggest component (plan §8.3)", () => {
  it("prerender safety: closed store renders zero dialog DOM", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    await settle();
    expect(harness.localeSuggest.open).toBe(false);
    expect(dialog()).toBeNull();
    expect(document.querySelector('[data-slot="locale-suggest-overlay"]')).toBeNull();
  });

  it("renders the endonym pitch and both actions in the current locale", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    expect(dialog()?.textContent ?? "").toContain("EasyQuran is also available in");
    expect(dialog()?.textContent ?? "").toContain("العربية");
    expect(findButton("Not now")).not.toBeNull();
    // The message ships the endonym pre-wrapped in Unicode isolates (plan §5.2).
    const linkText = acceptLink()?.textContent ?? "";
    expect(linkText).toContain("Switch to");
    expect(linkText).toContain("العربية");
    expect(dialog()?.getAttribute("dir")).toBe("ltr");
    expect(dialog()?.getAttribute("lang")).toBe("en");
  });

  it("the accept link is a full-reload href with query and hash; accepting persists", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/al-fatihah/t/en/sahih", "?v=1", "#2:255");
    await settle();
    const link = acceptLink();
    expect(link?.getAttribute("href")).toBe("/ar/al-fatihah/t/en/sahih?v=1#2:255");
    expect(link?.getAttribute("hreflang")).toBe("ar");
    link?.click();
    await settle();
    expect(stored()).toEqual({ ar: "accepted" });
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_accepted", { suggested_locale: "ar" });
  });

  it('"Not now" writes dismissed exactly once and closes; later evaluate is a no-op', async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    const setItem = vi.spyOn(localStorage, "setItem");
    findButton("Not now")?.click();
    await settle();
    expect(stored()).toEqual({ ar: "dismissed" });
    expect(setItem).toHaveBeenCalledTimes(1);
    expect(dialog()).toBeNull();
    harness.localeSuggest.evaluate("/search", "", "");
    expect(harness.localeSuggest.open).toBe(false);
  });

  it("the ✕ close icon funnels to a dismissal with its own reason", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    findButton("Dismiss")?.click();
    await settle();
    expect(stored()).toEqual({ ar: "dismissed" });
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_dismissed", {
      suggested_locale: "ar",
      dismiss_reason: "close_icon",
    });
    expect(harness.localeSuggest.open).toBe(false);
  });

  it("Escape writes a dismissal (explicit, not implied)", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await settle();
    expect(stored()).toEqual({ ar: "dismissed" });
    expect(trackMock).toHaveBeenCalledWith("locale_suggest_dismissed", {
      suggested_locale: "ar",
      dismiss_reason: "escape",
    });
    expect(harness.localeSuggest.open).toBe(false);
  });

  it("an auth modal opening on top force-closes without writing", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    expect(dialog()).not.toBeNull();
    harness.authModal.show("login");
    await settle();
    expect(harness.localeSuggest.open).toBe(false);
    expect(dialog()).toBeNull();
    expect(localStorage.getItem("easyquran.localeSuggest")).toBeNull();
    expect(trackMock).not.toHaveBeenCalledWith("locale_suggest_dismissed", expect.anything());
  });

  it("a cross-tab storage event closes the open dialog without writing", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    localStorage.setItem(
      "easyquran.localeSuggest",
      JSON.stringify({ v: 1, resolved: { ar: "dismissed" } }),
    );
    const setItem = vi.spyOn(localStorage, "setItem");
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.localeSuggest" }));
    await settle();
    expect(harness.localeSuggest.open).toBe(false);
    expect(setItem).not.toHaveBeenCalled();
  });

  it("blocked storage never throws: the dismissal lands in memory only", async () => {
    const harness = await freshHarness();
    target = harness.target;
    dispose = harness.dispose;
    stubArabicBrowser();
    harness.localeSuggest.hydrate();
    harness.localeSuggest.evaluate("/", "", "");
    await settle();
    vi.spyOn(localStorage, "setItem").mockImplementation(() => {
      throw new Error("quota");
    });
    expect(() => findButton("Not now")?.click()).not.toThrow();
    await settle();
    expect(harness.localeSuggest.open).toBe(false);
    harness.localeSuggest.evaluate("/search", "", "");
    expect(harness.localeSuggest.open).toBe(false);
  });
});
