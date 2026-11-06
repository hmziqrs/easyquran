import { describe, expect, it, vi } from "vite-plus/test";

import {
  MAX_REPEAT_PRESETS,
  addPreset,
  normalizePlan,
  parsePresets,
  planSteps,
  removePreset,
  runRepeat,
  waitMs,
  type RepeatPreset,
  type RepeatStep,
} from "../repeat";

const range3 = { surah: 1, from: 1, to: 3, ayahRepeat: 1, setRepeat: 2, delayMs: 250 };

describe("normalizePlan", () => {
  it("accepts a valid 3-verse range", () => {
    expect(normalizePlan(range3, 7)).toEqual(range3);
  });

  it("clamps the end ayah to the surah length", () => {
    expect(normalizePlan({ ...range3, from: 5, to: 99 }, 7)?.to).toBe(7);
  });

  it("refuses an inverted or out-of-surah start", () => {
    expect(normalizePlan({ ...range3, from: 3, to: 2 }, 7)).toBeNull();
    expect(normalizePlan({ ...range3, from: 8, to: 9 }, 7)).toBeNull();
    expect(normalizePlan({ ...range3, from: 0 }, 7)).toBeNull();
  });

  it("refuses an invalid surah", () => {
    expect(normalizePlan({ ...range3, surah: 115 }, 7)).toBeNull();
  });

  it("clamps repeat counts and delay into their limits", () => {
    const plan = normalizePlan({ ...range3, ayahRepeat: 500, setRepeat: 0, delayMs: -4 }, 7);
    expect(plan?.ayahRepeat).toBe(20);
    expect(plan?.setRepeat).toBe(1);
    expect(plan?.delayMs).toBe(0);
  });

  it("refuses a run past the step cap", () => {
    expect(
      normalizePlan(
        { surah: 2, from: 1, to: 286, ayahRepeat: 20, setRepeat: 100, delayMs: 0 },
        286,
      ),
    ).toBeNull();
  });
});

describe("planSteps", () => {
  it("orders set passes outside, ayahs in order, ayah repeats inside", () => {
    const plan = normalizePlan({ ...range3, ayahRepeat: 2, setRepeat: 2 }, 7);
    const steps = plan ? planSteps(plan) : [];
    expect(steps).toHaveLength(12);
    expect(steps.slice(0, 4).map((s) => [s.pass, s.ayah, s.ayahPass])).toEqual([
      [1, 1, 1],
      [1, 1, 2],
      [1, 2, 1],
      [1, 2, 2],
    ]);
    expect(steps[11]).toMatchObject({ pass: 2, ayah: 3, ayahPass: 2, ordinal: 11, total: 12 });
  });
});

describe("runRepeat", () => {
  function fakeIo() {
    const plays: RepeatStep[] = [];
    const waits: number[] = [];
    return {
      plays,
      waits,
      io: {
        play: vi.fn(async (step: RepeatStep) => {
          plays.push(step);
        }),
        wait: vi.fn(async (ms: number) => {
          waits.push(ms);
        }),
      },
    };
  }

  it("plays a 3-verse range for the set count with the delay between plays and stops cleanly", async () => {
    const plan = normalizePlan(range3, 7);
    const steps = plan ? planSteps(plan) : [];
    const { plays, waits, io } = fakeIo();
    const controller = new AbortController();
    const result = await runRepeat(steps, 250, io, controller.signal);
    expect(result).toBe("done");
    expect(plays.map((s) => s.ayah)).toEqual([1, 2, 3, 1, 2, 3]);
    // One delay between each consecutive pair, none after the last play.
    expect(waits).toEqual([250, 250, 250, 250, 250]);
  });

  it("skips waits when the delay is zero", async () => {
    const plan = normalizePlan(range3, 7);
    const { waits, io } = fakeIo();
    await runRepeat(plan ? planSteps(plan) : [], 0, io, new AbortController().signal);
    expect(waits).toEqual([]);
  });

  it("stops with no further play once aborted during a wait", async () => {
    const plan = normalizePlan(range3, 7);
    const { plays, io } = fakeIo();
    const controller = new AbortController();
    const cancelling = {
      ...io,
      wait: vi.fn(async () => {
        controller.abort();
      }),
    };
    const result = await runRepeat(plan ? planSteps(plan) : [], 250, cancelling, controller.signal);
    expect(result).toBe("cancelled");
    expect(plays).toHaveLength(1);
  });

  it("does not start a play when already aborted", async () => {
    const plan = normalizePlan(range3, 7);
    const { plays, io } = fakeIo();
    const controller = new AbortController();
    controller.abort();
    const result = await runRepeat(plan ? planSteps(plan) : [], 0, io, controller.signal);
    expect(result).toBe("cancelled");
    expect(plays).toHaveLength(0);
  });

  it("propagates a play failure and starts no further play", async () => {
    const plan = normalizePlan(range3, 7);
    const { plays, io } = fakeIo();
    let calls = 0;
    const failing = {
      ...io,
      play: vi.fn(async (step: RepeatStep) => {
        calls += 1;
        if (calls === 2) throw new Error("load");
        plays.push(step);
      }),
    };
    await expect(
      runRepeat(plan ? planSteps(plan) : [], 0, failing, new AbortController().signal),
    ).rejects.toThrow("load");
    expect(plays).toHaveLength(1);
  });
});

describe("waitMs", () => {
  it("resolves immediately once the signal is already aborted", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(waitMs(60_000, controller.signal)).resolves.toBeUndefined();
  });
});

describe("saved presets", () => {
  const preset = (id: string, surah = 1): RepeatPreset => ({
    id,
    name: `p${id}`,
    surah,
    from: 1,
    to: 3,
    ayahRepeat: 1,
    setRepeat: 1,
    delayMs: 0,
  });

  it("keeps only well-formed presets", () => {
    const good = {
      id: "a",
      name: "pa",
      surah: 1,
      from: 1,
      to: 3,
      ayahRepeat: 1,
      setRepeat: 1,
      delayMs: 0,
    };
    const parsed = parsePresets([good, { id: "bad" }, "nope", null]);
    expect(parsed.map((p) => p.id)).toEqual(["a"]);
    expect(parsePresets({ not: "an array" })).toEqual([]);
  });

  it("drops a preset whose repeat count is out of range", () => {
    const tooMany = {
      id: "b",
      name: "pb",
      surah: 1,
      from: 1,
      to: 3,
      ayahRepeat: 500,
      setRepeat: 1,
      delayMs: 0,
    };
    expect(parsePresets([tooMany])).toEqual([]);
  });

  it("adds at the front and caps the list", () => {
    let list: RepeatPreset[] = [];
    for (let i = 0; i < MAX_REPEAT_PRESETS + 3; i++) list = addPreset(list, preset(`${i}`));
    expect(list).toHaveLength(MAX_REPEAT_PRESETS);
    expect(list[0]?.id).toBe(`${MAX_REPEAT_PRESETS + 2}`);
  });

  it("replaces a preset with the same id instead of duplicating it", () => {
    const list = addPreset([preset("a")], { ...preset("a"), name: "renamed" });
    expect(list).toHaveLength(1);
    expect(list[0]?.name).toBe("renamed");
  });

  it("removes by id", () => {
    expect(removePreset([preset("a"), preset("b")], "a").map((p) => p.id)).toEqual(["b"]);
  });
});
