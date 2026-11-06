import { describe, it, expect, beforeEach, afterEach, vi } from "vite-plus/test";

const flag = vi.hoisted(() => ({ value: true }));
vi.mock("$app/env", () => ({
  get browser() {
    return flag.value;
  },
}));

import {
  ARABIC_SELECTION,
  PINS_MAX,
  PinsStore,
  compareCell,
  compareSelections,
  decodePins,
  type PinnedVerse,
} from "../pins.svelte";

const KEY = "easyquran.reader.pins";

const verse = (key: string, overrides: Partial<PinnedVerse> = {}): PinnedVerse => ({
  key,
  sourceId: null,
  text: `arabic-${key}`,
  lanes: [],
  ...overrides,
});

describe("PinsStore", () => {
  let store: PinsStore;

  beforeEach(() => {
    flag.value = true;
    window.localStorage.clear();
    store = new PinsStore();
  });
  afterEach(() => store.dispose());

  const read = (): { v: number; pins: PinnedVerse[] } | null =>
    JSON.parse(window.localStorage.getItem(KEY) ?? "null");

  it("starts empty and writes nothing until a pin is made", () => {
    expect(store.count).toBe(0);
    expect(read()).toBeNull();
  });

  it("pins in order, toggles off on a second pin, and persists", () => {
    expect(store.toggle(verse("2:255"))).toBe("added");
    expect(store.toggle(verse("1:1"))).toBe("added");
    expect(store.list.map((p) => p.key)).toEqual(["2:255", "1:1"]);
    expect(store.has("1:1")).toBe(true);

    expect(store.toggle(verse("1:1"))).toBe("removed");
    expect(store.has("1:1")).toBe(false);

    expect(read()?.pins.map((p) => p.key)).toEqual(["2:255"]);
    const fresh = new PinsStore();
    expect(fresh.list.map((p) => p.key)).toEqual(["2:255"]);
    fresh.dispose();
  });

  it("keeps pins across a fresh store read, the navigation case", () => {
    store.toggle(verse("3:7", { sourceId: "en.sahih", text: "english line" }));
    const after = new PinsStore();
    expect(after.list[0]).toMatchObject({ key: "3:7", sourceId: "en.sahih", text: "english line" });
    after.dispose();
  });

  it("refuses a pin beyond the cap and reports full", () => {
    for (let i = 1; i <= PINS_MAX; i++) store.toggle(verse(`1:${i}`));
    expect(store.count).toBe(PINS_MAX);
    expect(store.toggle(verse("2:1"))).toBe("full");
    expect(store.has("2:1")).toBe(false);
  });

  it("moves a pin up and down, edges are no-ops", () => {
    store.toggle(verse("1:1"));
    store.toggle(verse("1:2"));
    store.toggle(verse("1:3"));
    store.move("1:3", -1);
    expect(store.list.map((p) => p.key)).toEqual(["1:1", "1:3", "1:2"]);
    store.move("1:1", -1);
    expect(store.list.map((p) => p.key)).toEqual(["1:1", "1:3", "1:2"]);
    store.move("1:2", 1);
    expect(store.list.map((p) => p.key)).toEqual(["1:1", "1:3", "1:2"]);
    store.move("1:1", 1);
    expect(store.list.map((p) => p.key)).toEqual(["1:3", "1:1", "1:2"]);
  });

  it("removes one pin and clears all", () => {
    store.toggle(verse("1:1"));
    store.toggle(verse("1:2"));
    store.remove("1:1");
    expect(store.list.map((p) => p.key)).toEqual(["1:2"]);
    store.clear();
    expect(store.count).toBe(0);
    expect(read()).toEqual({ v: 1, pins: [] });
  });

  it("is memory-only when not in a browser", () => {
    flag.value = false;
    const memory = new PinsStore();
    memory.toggle(verse("1:1"));
    expect(memory.count).toBe(1);
    expect(window.localStorage.getItem(KEY)).toBeNull();
    memory.dispose();
  });
});

describe("decodePins", () => {
  it("drops malformed rows, bad keys and duplicate keys", () => {
    const decoded = decodePins({
      v: 1,
      pins: [
        { key: "2:255", sourceId: null, text: "a", lanes: [] },
        { key: "2:255", sourceId: null, text: "dup", lanes: [] },
        { key: "not-a-key", sourceId: null, text: "x", lanes: [] },
        { key: "1:1", sourceId: null },
        "nonsense",
      ],
    });
    expect(decoded.map((p) => p.key)).toEqual(["2:255"]);
    expect(decoded[0]?.text).toBe("a");
  });

  it("treats a future schema as empty", () => {
    expect(decodePins({ v: 99, pins: [{ key: "1:1", text: "a" }] })).toEqual([]);
  });

  it("keeps only well-formed lanes", () => {
    const [pin] = decodePins({
      v: 1,
      pins: [
        {
          key: "1:2",
          sourceId: "en.sahih",
          text: "t",
          lanes: [
            { sourceId: "ur.jalandhry", name: "U", text: "ok" },
            { sourceId: "", name: "bad", text: "no id" },
            { sourceId: "ms.basmeih", name: "M" },
          ],
        },
      ],
    });
    expect(pin?.lanes.map((l) => l.sourceId)).toEqual(["ur.jalandhry"]);
  });
});

describe("compare composition", () => {
  const pins: PinnedVerse[] = [
    verse("2:255", {
      lanes: [{ sourceId: "en.sahih", name: "Saheeh", text: "The Throne verse" }],
    }),
    verse("1:1", {
      sourceId: "en.sahih",
      text: "In the name of Allah",
      lanes: [{ sourceId: "ur.jalandhry", name: "Jalandhry", text: "urdu" }],
    }),
  ];

  it("offers Arabic first, then each translation in first-seen order", () => {
    expect(compareSelections(pins)).toEqual([ARABIC_SELECTION, "en.sahih", "ur.jalandhry"]);
  });

  it("composes Arabic with the selected translation, text unchanged", () => {
    expect(compareCell(pins[0]!, "en.sahih")).toEqual({
      key: "2:255",
      arabic: "arabic-2:255",
      translation: "The Throne verse",
    });
  });

  it("shows a null translation when the pin lacks the selection", () => {
    expect(compareCell(pins[0]!, "ur.jalandhry")).toEqual({
      key: "2:255",
      arabic: "arabic-2:255",
      translation: null,
    });
  });

  it("uses the pinned text as the translation when the route translation is selected", () => {
    expect(compareCell(pins[1]!, "en.sahih")).toEqual({
      key: "1:1",
      arabic: null,
      translation: "In the name of Allah",
    });
  });

  it("selects Arabic only, with no translation", () => {
    expect(compareCell(pins[1]!, ARABIC_SELECTION)).toEqual({
      key: "1:1",
      arabic: null,
      translation: null,
    });
  });
});
