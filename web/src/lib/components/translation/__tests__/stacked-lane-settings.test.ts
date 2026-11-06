import { mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

vi.mock("$app/env", () => ({ browser: true, dev: false }));
vi.mock("$app/env/public", () => ({
  PUBLIC_API_BASE_URL: undefined,
  PUBLIC_QURAN_API_BASE: undefined,
  PUBLIC_ENV: undefined,
  PUBLIC_FCM_VAPID_KEY: undefined,
  PUBLIC_TURNSTILE_SITE_KEY: undefined,
}));

import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";

import StackedLaneSettings from "../StackedLaneSettings.svelte";

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

async function render(): Promise<void> {
  app = mount(StackedLaneSettings, { target });
  await tick();
}

const button = (selector: string): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>(selector);

const laneRow = (id: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[data-lane-size="${id}"]`);

beforeEach(() => {
  target = document.createElement("div");
  document.body.append(target);
  stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
  stackedTranslations.setLayout("stacked");
});

afterEach(async () => {
  if (app) await unmount(app);
  app = null;
  target.remove();
  stackedTranslations.setIds([]);
  stackedTranslations.setLayout("stacked");
});

describe("StackedLaneSettings", () => {
  it("lists each selected translation with its own size controls", async () => {
    await render();
    expect(laneRow("en.sahih")).not.toBeNull();
    expect(laneRow("ur.jalandhry")).not.toBeNull();
    expect(document.querySelectorAll("[data-lane-size]")).toHaveLength(2);
  });

  it("switches the layout between stacked and side by side", async () => {
    await render();
    const parallel = button('[data-layout="parallel"]');
    expect(parallel?.getAttribute("aria-pressed")).toBe("false");

    parallel?.click();
    await tick();
    expect(stackedTranslations.layout).toBe("parallel");
    expect(button('[data-layout="parallel"]')?.getAttribute("aria-pressed")).toBe("true");
    expect(button('[data-layout="stacked"]')?.getAttribute("aria-pressed")).toBe("false");

    button('[data-layout="stacked"]')?.click();
    await tick();
    expect(stackedTranslations.layout).toBe("stacked");
  });

  it("makes one lane larger and smaller without touching the other", async () => {
    await render();
    const row = laneRow("en.sahih");
    row?.querySelector<HTMLButtonElement>("[data-lane-larger]")?.click();
    await tick();
    expect(stackedTranslations.laneStep("en.sahih")).toBe(1);
    expect(stackedTranslations.laneStep("ur.jalandhry")).toBe(0);

    row?.querySelector<HTMLButtonElement>("[data-lane-smaller]")?.click();
    row?.querySelector<HTMLButtonElement>("[data-lane-smaller]")?.click();
    await tick();
    expect(stackedTranslations.laneStep("en.sahih")).toBe(-1);
  });

  it("disables a control at the end of the size range", async () => {
    await render();
    const row = laneRow("en.sahih");
    const larger = row?.querySelector<HTMLButtonElement>("[data-lane-larger]");
    larger?.click();
    larger?.click();
    await tick();
    expect(stackedTranslations.laneStep("en.sahih")).toBe(2);
    expect(larger?.disabled).toBe(true);
  });

  it("resets one lane to the reader size, and only enables reset when stepped", async () => {
    await render();
    const row = laneRow("ur.jalandhry");
    const reset = row?.querySelector<HTMLButtonElement>("[data-lane-reset]");
    expect(reset?.disabled).toBe(true);

    row?.querySelector<HTMLButtonElement>("[data-lane-smaller]")?.click();
    await tick();
    expect(reset?.disabled).toBe(false);

    reset?.click();
    await tick();
    expect(stackedTranslations.laneStep("ur.jalandhry")).toBe(0);
  });
});
