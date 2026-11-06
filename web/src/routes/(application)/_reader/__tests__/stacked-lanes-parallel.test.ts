import { mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

const media = vi.hoisted(() => ({ wide: true }));

vi.mock("$app/env", () => ({ browser: true, dev: false }));

import type { StackedTranslation } from "#lib/data/quran-types.js";
import { stackedTranslations } from "#lib/stores/stacked-translations.svelte.js";

import StackedLanes from "../StackedLanes.svelte";

const lane = (sourceId: string, text: string, languageCode = "en"): StackedTranslation => ({
  sourceId,
  name: `N-${sourceId}`,
  translator: `T-${sourceId}`,
  language: "English",
  languageCode,
  direction: "ltr",
  text,
});

const FIVE = ["en.sahih", "ur.jalandhry", "ms.basmeih", "fr.hamidullah", "tr.diyanet"];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

/** Stub the viewport: the lanes read `(min-width: 768px)` through matchMedia. */
function stubViewport(wide: boolean): void {
  media.wide = wide;
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: (query: string) => ({
      matches: query.includes("min-width: 768px") ? media.wide : false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}

async function render(stacked: StackedTranslation[]): Promise<HTMLElement> {
  app = mount(StackedLanes, { target, props: { stacked } });
  await tick();
  await tick();
  const lanes = target.querySelector<HTMLElement>(".verse-lanes");
  if (lanes === null) throw new Error("lanes not rendered");
  return lanes;
}

beforeEach(() => {
  target = document.createElement("div");
  document.body.append(target);
  stubViewport(true);
  stackedTranslations.setIds([]);
  stackedTranslations.setLayout("stacked");
});

afterEach(async () => {
  if (app) await unmount(app);
  app = null;
  target.remove();
  stackedTranslations.setIds([]);
  stackedTranslations.setLayout("stacked");
});

describe("StackedLanes layout", () => {
  it("stacks the lanes by default", async () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    const lanes = await render([lane("en.sahih", "a"), lane("ur.jalandhry", "b")]);
    expect(lanes.dataset.lanesLayout).toBe("stacked");
    expect(lanes.classList.contains("verse-lanes--parallel")).toBe(false);
  });

  it("lays two or more lanes side by side on a wide screen when parallel is chosen", async () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    stackedTranslations.setLayout("parallel");
    const lanes = await render([lane("en.sahih", "a"), lane("ur.jalandhry", "b")]);
    expect(lanes.classList.contains("verse-lanes--parallel")).toBe(true);
    expect(lanes.dataset.lanesLayout).toBe("parallel");
    expect(lanes.style.getPropertyValue("--lane-count")).toBe("2");
  });

  it("works with the full five extras", async () => {
    stackedTranslations.setIds(FIVE);
    stackedTranslations.setLayout("parallel");
    const lanes = await render(FIVE.map((id) => lane(id, `text-${id}`)));
    expect(lanes.classList.contains("verse-lanes--parallel")).toBe(true);
    expect(lanes.style.getPropertyValue("--lane-count")).toBe("5");
    expect(lanes.querySelectorAll(".verse-extra")).toHaveLength(5);
  });

  it("collapses back to stacked on a narrow screen, keeping the lanes", async () => {
    stubViewport(false);
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    stackedTranslations.setLayout("parallel");
    const lanes = await render([lane("en.sahih", "a"), lane("ur.jalandhry", "b")]);
    expect(lanes.classList.contains("verse-lanes--parallel")).toBe(false);
    expect(lanes.querySelectorAll(".verse-extra")).toHaveLength(2);
  });

  it("keeps a single lane stacked even when parallel is chosen", async () => {
    stackedTranslations.setIds(["en.sahih"]);
    stackedTranslations.setLayout("parallel");
    const lanes = await render([lane("en.sahih", "a")]);
    expect(lanes.classList.contains("verse-lanes--parallel")).toBe(false);
  });

  it("applies each lane's own size step as its font scale", async () => {
    stackedTranslations.setIds(["en.sahih", "ur.jalandhry"]);
    stackedTranslations.setLayout("parallel");
    stackedTranslations.stepLane("en.sahih", 1);
    stackedTranslations.stepLane("ur.jalandhry", -1);
    const lanes = await render([lane("en.sahih", "a"), lane("ur.jalandhry", "b", "ur")]);
    const en = lanes.querySelector<HTMLElement>('.verse-extra[lang="en"]');
    const ur = lanes.querySelector<HTMLElement>('.verse-extra[lang="ur"]');
    expect(en?.style.getPropertyValue("--lane-scale")).toBe("1.125");
    expect(ur?.style.getPropertyValue("--lane-scale")).toBe("0.875");
  });
});
