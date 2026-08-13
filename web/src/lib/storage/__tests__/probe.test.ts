import { flushSync } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { describe, it, expect } from "vite-plus/test";

import { observeMapSize } from "./reactive.probe.svelte";

describe("probe", () => {
  it("runs and exposes vite-plus/test", () => {
    expect(1 + 1).toBe(2);
  });
  it("SvelteMap mutates readably in node env", () => {
    const m = new SvelteMap<number, string[]>();
    m.set(2, ["a", "b"]);
    expect(m.get(2)).toEqual(["a", "b"]);
  });
  it("SvelteMap size is reactive under effect.root + flushSync", () => {
    const m = new SvelteMap<number, string[]>();
    const obs = observeMapSize(m);
    flushSync();
    const initial = obs.reads();
    m.set(1, ["x"]);
    flushSync();
    expect(obs.reads()).toBeGreaterThan(initial);
    obs.dispose();
  });
});
