import { mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { bufferedIndexes, type ReaderVirtualItem } from "../virtual-reader";
import ReaderVirtualListHost from "./ReaderVirtualListHost.svelte";

const items: ReaderVirtualItem[] = Array.from({ length: 300 }, (_, index) => ({
  key: `2:${index + 1}`,
  verseKey: `2:${index + 1}`,
  localPage: Math.floor(index / 20) + 1,
  estimate: 80,
}));

let target: HTMLElement;
let view: ReturnType<typeof ReaderVirtualListHost> | undefined;

async function scrollToOffset(offset: number): Promise<void> {
  vi.stubGlobal("scrollY", offset);
  window.dispatchEvent(new Event("scroll"));
  await tick();
}

beforeEach(() => {
  target = document.createElement("div");
  document.body.appendChild(target);
  vi.stubGlobal("innerHeight", 800);
  vi.stubGlobal("innerWidth", 1200);
  vi.stubGlobal("scrollY", 0);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (
    this: HTMLElement,
  ) {
    return Number.parseFloat(this.style.height) || 0;
  });
});

afterEach(async () => {
  if (view) await unmount(view);
  view = undefined;
  target.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("reader virtualization", () => {
  it("covers viewport plus pixel buffer when every ayah is short", async () => {
    view = mount(ReaderVirtualListHost, { target, props: { items } });
    await tick();
    const rows = [...target.querySelectorAll<HTMLElement>("[data-index]")];
    expect(Number(rows.at(-1)?.dataset.index)).toBeGreaterThanOrEqual(18);
    expect(rows.length).toBeLessThan(30);
    await scrollToOffset(16000);
    const scrolled = [...target.querySelectorAll<HTMLElement>("[data-index]")];
    expect(Number(scrolled[0]?.dataset.index)).toBeLessThanOrEqual(192);
    expect(Number(scrolled.at(-1)?.dataset.index)).toBeGreaterThanOrEqual(218);
    expect(target.querySelector('[data-index="0"]')).toBeNull();
    expect(scrolled.length).toBeLessThan(30);
  });

  it("keeps focused control mounted without mounting intervening ayahs", async () => {
    view = mount(ReaderVirtualListHost, { target, props: { items } });
    await tick();
    const button = target.querySelector<HTMLButtonElement>('[data-index="0"] button');
    button?.focus();
    await tick();
    await scrollToOffset(16000);
    expect(document.activeElement).toBe(button);
    expect(button?.isConnected).toBe(true);
    expect(target.querySelector('[data-index="100"]')).toBeNull();
    const rows = [...target.querySelectorAll<HTMLElement>("[data-index]")];
    expect(rows.length).toBeLessThan(31);
    expect(Number.parseFloat(rows[1]?.style.marginBlockStart ?? "0")).toBeGreaterThan(10000);
    button?.blur();
    await new Promise<void>((resolveMicrotask) => queueMicrotask(resolveMicrotask));
    await tick();
    expect(target.querySelector('[data-index="0"]')).toBeNull();
  });

  it("bounds edge ranges and includes one distant focused item once", () => {
    expect(bufferedIndexes(-4, 3, 10, 9)).toEqual([0, 1, 2, 3, 9]);
    expect(bufferedIndexes(7, 20, 10, 8)).toEqual([7, 8, 9]);
    expect(bufferedIndexes(0, 0, 0, 0)).toEqual([]);
  });

  it("fills tall viewport without fixed item cap", async () => {
    vi.stubGlobal("innerHeight", 3000);
    view = mount(ReaderVirtualListHost, { target, props: { items } });
    await tick();
    const rows = [...target.querySelectorAll<HTMLElement>("[data-index]")];
    expect(Number(rows.at(-1)?.dataset.index)).toBeGreaterThanOrEqual(52);
    expect(rows.length).toBeLessThan(65);
  });

  it("materializes distant target before anchor lookup", async () => {
    vi.spyOn(document.documentElement, "scrollHeight", "get").mockReturnValue(24000);
    vi.spyOn(window, "scrollTo").mockImplementation(
      (options: ScrollToOptions | number, y?: number) => {
        const offset = options instanceof Object ? (options.top ?? 0) : (y ?? 0);
        vi.stubGlobal("scrollY", offset);
        window.dispatchEvent(new Event("scroll"));
      },
    );
    view = mount(ReaderVirtualListHost, { target, props: { items } });
    await tick();
    expect(target.querySelector('[data-verse-key="2:251"]')).toBeNull();
    await view.reveal("2:251", 13);
    expect(target.querySelector('[data-verse-key="2:251"]')).not.toBeNull();
    expect(target.querySelectorAll("[data-index]").length).toBeLessThan(30);
  });
});
