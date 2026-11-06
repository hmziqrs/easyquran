import { mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => ({
  nav: {
    url: new URL("https://example.test/al-fatihah"),
    // SAFETY: empty params literal widens to the page.params Record<string,string> contract; tests never assign to it
    params: {} as Record<string, string>,
    state: {},
  },
  readerStub: {
    getNote: () => "",
    toggleNote: () => {},
    setNote: () => {},
    copyVerse: async () => true,
    shareVerse: async () => "copied",
  },
}));

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  PUBLIC_TURNSTILE_SITE_KEY: undefined,
}));
vi.mock("$app/state", () => ({ page: h.nav }));
vi.mock("#lib/stores/reader.svelte.js", () => ({ reader: h.readerStub }));
vi.mock("#lib/bookmarks/store.svelte.js", () => ({
  bookmarks: { isMarkedKey: () => false },
}));
vi.mock("#lib/bookmarks/feedback.svelte.js", () => ({ toggleBookmark: () => {} }));

import type { StackedTranslation } from "#lib/data/quran-types.js";
import { pins, PINS_MAX } from "#lib/stores/pins.svelte.js";

import VerseTools from "./VerseToolsHost.svelte";

const lane = (sourceId: string, text: string): StackedTranslation => ({
  sourceId,
  name: `N-${sourceId}`,
  translator: null,
  language: "English",
  languageCode: "en",
  direction: "ltr",
  text,
});

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

const pinButton = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>(
    'button[aria-pressed][aria-label="Pin verse"], button[aria-pressed][aria-label="Unpin verse"]',
  );

async function render(props: { text: string; vKey: string; lanes?: StackedTranslation[] }) {
  app = mount(VerseTools, { target, props });
  await tick();
}

beforeEach(() => {
  window.localStorage.clear();
  pins.clear();
  h.nav.params = {};
  target = document.createElement("div");
  document.body.append(target);
});

afterEach(async () => {
  if (app) await unmount(app);
  app = null;
  target.remove();
  pins.clear();
});

describe("VerseTools pin toggle", () => {
  it("pins the verse with its row text and the lanes it shows, then unpins it", async () => {
    await render({
      text: "الحمد لله",
      vKey: "1:2",
      lanes: [lane("en.sahih", "Praise be to Allah")],
    });

    expect(pinButton()?.getAttribute("aria-pressed")).toBe("false");
    pinButton()?.click();
    await tick();

    expect(pins.list).toEqual([
      {
        key: "1:2",
        sourceId: null,
        text: "الحمد لله",
        lanes: [{ sourceId: "en.sahih", name: "N-en.sahih", text: "Praise be to Allah" }],
      },
    ]);
    expect(pinButton()?.getAttribute("aria-pressed")).toBe("true");
    expect(pinButton()?.getAttribute("aria-label")).toBe("Unpin verse");

    pinButton()?.click();
    await tick();
    expect(pins.list).toEqual([]);
    expect(pinButton()?.getAttribute("aria-pressed")).toBe("false");
  });

  it("records the route translation as the pin's source on a translation route", async () => {
    h.nav.params = { lang: "en", translator: "sahih" };
    await render({ text: "Praise be to Allah", vKey: "1:2" });

    pinButton()?.click();
    await tick();

    expect(pins.list[0]?.sourceId).not.toBeNull();
    expect(pins.list[0]?.text).toBe("Praise be to Allah");
  });

  it("says so when the pin set is full and keeps the verse out", async () => {
    for (let i = 1; i <= PINS_MAX; i++) {
      pins.toggle({ key: `2:${i}`, sourceId: null, text: "x", lanes: [] });
    }
    await render({ text: "الحمد لله", vKey: "1:2" });

    pinButton()?.click();
    await tick();

    expect(pins.has("1:2")).toBe(false);
    expect(document.body.textContent).toContain(`Pin limit reached (${PINS_MAX})`);
  });
});
