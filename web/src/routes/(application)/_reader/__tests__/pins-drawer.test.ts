import { mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const h = vi.hoisted(() => ({
  createFolder: vi.fn((_name: string) => ({ id: "folder-1" })),
  setInFolder: vi.fn(
    (
      _surah: number,
      _ayah: number,
      _folderId: string,
      _on: boolean,
      _sourceId: string | null,
    ) => {},
  ),
}));

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  PUBLIC_TURNSTILE_SITE_KEY: undefined,
}));
vi.mock("#lib/bookmarks/store.svelte.js", () => ({ bookmarks: h }));

import { pins, type PinnedVerse } from "#lib/stores/pins.svelte.js";

import PinsDrawer from "../PinsDrawer.svelte";

const verse = (key: string, overrides: Partial<PinnedVerse> = {}): PinnedVerse => ({
  key,
  sourceId: null,
  text: `arabic-${key}`,
  lanes: [],
  ...overrides,
});

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

async function render(): Promise<void> {
  app = mount(PinsDrawer, { target });
  await tick();
}

const trigger = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>("[data-pins-trigger]");

async function openDrawer(): Promise<void> {
  trigger()?.click();
  await tick();
  await tick();
}

const listKeys = (): string[] =>
  Array.from(document.querySelectorAll("[data-pins-list] li .truncate")).map(
    (el) => el.textContent?.trim() ?? "",
  );

beforeEach(() => {
  window.localStorage.clear();
  h.createFolder.mockClear();
  h.setInFolder.mockClear();
  target = document.createElement("div");
  document.body.append(target);
  // Clear pins left by an earlier test in this file.
  pins.clear();
});

afterEach(async () => {
  if (app) await unmount(app);
  app = null;
  target.remove();
  document.body.innerHTML = "";
  pins.clear();
});

describe("PinsDrawer", () => {
  it("shows no trigger until a verse is pinned, then the count", async () => {
    await render();
    expect(trigger()).toBeNull();

    pins.toggle(verse("2:255"));
    await tick();
    expect(trigger()?.getAttribute("aria-label")).toBe("Pinned verses (1)");
    expect(trigger()?.textContent?.trim()).toBe("1");
  });

  it("lists pins in order and reorders them with the move controls", async () => {
    pins.toggle(verse("1:1"));
    pins.toggle(verse("2:255"));
    await render();
    await openDrawer();

    expect(listKeys()).toEqual(["1:1", "2:255"]);
    const moveDown = document.querySelector<HTMLButtonElement>(
      '[data-pins-list] li:first-child button[aria-label="Move down"]',
    );
    moveDown?.click();
    await tick();
    expect(pins.list.map((p) => p.key)).toEqual(["2:255", "1:1"]);
    expect(listKeys()).toEqual(["2:255", "1:1"]);
  });

  it("composes Arabic with the selected translation and keeps the text verbatim", async () => {
    pins.toggle(
      verse("2:255", {
        lanes: [{ sourceId: "en.sahih", name: "Saheeh", text: "Allah - there is no deity" }],
      }),
    );
    await render();
    await openDrawer();

    const cell = document.querySelector<HTMLElement>('[data-pin-cell="2:255"]');
    expect(cell?.textContent).toContain("arabic-2:255");
    expect(cell?.textContent).not.toContain("Allah - there is no deity");

    const select = document.querySelector<HTMLSelectElement>("[data-pins-selection]");
    expect(select).not.toBeNull();
    if (select) {
      select.value = "en.sahih";
      // Svelte delegates change; a real change event bubbles, so the test event must too.
      select.dispatchEvent(new Event("change", { bubbles: true }));
    }
    await tick();
    expect(document.querySelector<HTMLElement>('[data-pin-cell="2:255"]')?.textContent).toContain(
      "Allah - there is no deity",
    );
  });

  it("says so when a pin was not made with the chosen translation", async () => {
    pins.toggle(verse("1:1"));
    pins.toggle(
      verse("2:255", {
        lanes: [{ sourceId: "en.sahih", name: "Saheeh", text: "throne" }],
      }),
    );
    await render();
    await openDrawer();

    const select = document.querySelector<HTMLSelectElement>("[data-pins-selection]");
    if (select) {
      select.value = "en.sahih";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    }
    await tick();
    expect(document.querySelector<HTMLElement>('[data-pin-cell="1:1"]')?.textContent).toContain(
      "Not pinned with this translation.",
    );
  });

  it("saves every pin into one new bookmark folder with its translation context", async () => {
    pins.toggle(verse("1:1"));
    pins.toggle(verse("2:255", { sourceId: "en.sahih", text: "throne" }));
    await render();
    await openDrawer();

    document.querySelector<HTMLButtonElement>("[data-pins-save]")?.click();
    await tick();

    expect(h.createFolder).toHaveBeenCalledTimes(1);
    expect(h.createFolder).toHaveBeenCalledWith("Pinned verses");
    expect(h.setInFolder).toHaveBeenCalledTimes(2);
    expect(h.setInFolder).toHaveBeenNthCalledWith(1, 1, 1, "folder-1", true, null);
    expect(h.setInFolder).toHaveBeenNthCalledWith(2, 2, 255, "folder-1", true, "en.sahih");
    expect(document.querySelector("[role=status]")?.textContent).toContain("Saved 2 verses");
  });

  it("shows the empty state inside the drawer when the set is cleared", async () => {
    pins.toggle(verse("1:1"));
    await render();
    await openDrawer();
    pins.clear();
    await tick();
    expect(document.querySelector("[data-pins-list]")).toBeNull();
    expect(document.body.textContent).toContain("No pinned verses yet.");
  });
});
