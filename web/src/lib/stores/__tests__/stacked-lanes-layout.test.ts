import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/env", () => ({
  get browser() {
    return flag.value;
  },
}));

import {
  LANE_STEP_MAX,
  LANE_STEP_MIN,
  StackedTranslationsStore,
} from "../stacked-translations.svelte";

const KEY = "easyquran.reader.stacked";

type Persisted = {
  v: number;
  ids: string[];
  enabled?: boolean;
  layout?: string;
  steps?: Record<string, number>;
};

describe("StackedTranslationsStore layout and lane sizes", () => {
  let store: StackedTranslationsStore;

  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
    store = new StackedTranslationsStore();
    store.setIds(["en.sahih", "ur.jalandhry"]);
  });
  afterEach(() => store.dispose());

  const read = (): Persisted | null => {
    // SAFETY: the test reads back exactly what the store wrote under KEY; each assertion checks the fields it uses.
    const parsed = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as Persisted | null;
    return parsed;
  };

  it("defaults to stacked with every lane at the reader size, and writes no extra fields", () => {
    expect(store.layout).toBe("stacked");
    expect(store.laneStep("en.sahih")).toBe(0);
    expect(store.laneScale("en.sahih")).toBe(1);
    expect(read()).toEqual({ v: 1, ids: ["en.sahih", "ur.jalandhry"], enabled: true });
  });

  it("persists side by side, and drops back to the default key when stacked again", () => {
    store.setLayout("parallel");
    expect(store.layout).toBe("parallel");
    expect(read()?.layout).toBe("parallel");

    const fresh = new StackedTranslationsStore();
    expect(fresh.layout).toBe("parallel");
    fresh.dispose();

    store.setLayout("stacked");
    expect(read()).not.toHaveProperty("layout");
  });

  it("steps one lane up and down within the range and leaves others alone", () => {
    store.stepLane("en.sahih", 1);
    expect(store.laneStep("en.sahih")).toBe(1);
    expect(store.laneScale("en.sahih")).toBe(1.125);
    expect(store.laneStep("ur.jalandhry")).toBe(0);
    expect(read()?.steps).toEqual({ "en.sahih": 1 });

    store.stepLane("en.sahih", -1);
    store.stepLane("en.sahih", -1);
    expect(store.laneStep("en.sahih")).toBe(-1);
    expect(store.laneScale("en.sahih")).toBe(0.875);
  });

  it("clamps lane steps at the smallest and largest size", () => {
    for (let i = 0; i < 10; i++) store.stepLane("en.sahih", 1);
    expect(store.laneStep("en.sahih")).toBe(LANE_STEP_MAX);
    expect(store.laneScale("en.sahih")).toBe(1.25);

    for (let i = 0; i < 10; i++) store.stepLane("ur.jalandhry", -1);
    expect(store.laneStep("ur.jalandhry")).toBe(LANE_STEP_MIN);
    expect(store.laneScale("ur.jalandhry")).toBe(0.75);
  });

  it("ignores a size step for a lane that is not selected", () => {
    store.stepLane("ms.basmeih", 1);
    expect(store.laneStep("ms.basmeih")).toBe(0);
    expect(read()?.steps).toBeUndefined();
  });

  it("resets one lane to the reader size", () => {
    store.stepLane("en.sahih", 1);
    store.stepLane("ur.jalandhry", -1);
    store.resetLane("en.sahih");
    expect(store.laneStep("en.sahih")).toBe(0);
    expect(store.laneStep("ur.jalandhry")).toBe(-1);
    expect(read()?.steps).toEqual({ "ur.jalandhry": -1 });
  });

  it("forgets a removed lane's size, so a re-added lane starts at the reader size", () => {
    store.stepLane("en.sahih", 1);
    store.remove("en.sahih");
    expect(read()?.steps).toBeUndefined();
    store.toggle("en.sahih");
    expect(store.laneStep("en.sahih")).toBe(0);
  });

  it("keeps the layout and sizes when the stack is paused and resumed", () => {
    store.setLayout("parallel");
    store.stepLane("en.sahih", 1);
    store.setEnabled(false);
    store.setEnabled(true);
    expect(store.layout).toBe("parallel");
    expect(store.laneStep("en.sahih")).toBe(1);
  });

  it("decodes a stored layout and sizes defensively", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        v: 1,
        ids: ["en.sahih", "ur.jalandhry"],
        layout: "diagonal",
        steps: { "en.sahih": 9, "ur.jalandhry": -1, "ms.basmeih": 2 },
      }),
    );
    const fresh = new StackedTranslationsStore();
    expect(fresh.layout).toBe("stacked");
    expect(fresh.laneStep("en.sahih")).toBe(0);
    expect(fresh.laneStep("ur.jalandhry")).toBe(-1);
    expect(fresh.laneStep("ms.basmeih")).toBe(0);
    fresh.dispose();
  });

  it("follows another tab's layout and size changes", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        v: 1,
        ids: ["en.sahih", "ur.jalandhry"],
        layout: "parallel",
        steps: { "en.sahih": 2 },
      }),
    );
    // Simulate the storage event the other tab would fire.
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(store.layout).toBe("parallel");
    expect(store.laneStep("en.sahih")).toBe(2);
  });
});
