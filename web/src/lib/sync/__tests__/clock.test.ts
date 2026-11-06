import { describe, expect, it } from "vite-plus/test";

import { createClock, type ClockState, type ClockStorage } from "../clock";
import { compareStamps, mergeRegister, parseStamp } from "../merge";

const HOUR = 3_600_000;
const SERVER = 1_800_000_000_000;

function sharedStorage(): ClockStorage {
  let state: ClockState | undefined;
  return {
    read: () => state,
    write: (next) => {
      state = next;
    },
  };
}

function device(id: string, skewMs: number, storage = sharedStorage(), base = { ms: SERVER }) {
  const clock = createClock(id, storage, () => base.ms + skewMs);
  return { clock, base };
}

describe("sync clock against the merge core", () => {
  it("a device 1 h slow, editing after a pull, beats what it saw", () => {
    const base = { ms: SERVER };
    const fast = device("fast", 0, sharedStorage(), base).clock;
    const slow = device("slow", -HOUR, sharedStorage(), base).clock;
    const seen = { stamp: fast.stamp(), value: "from another device" };
    slow.observe(seen.stamp);
    const edit = { stamp: slow.stamp(), value: "edited after the pull" };
    expect(mergeRegister(seen, edit)).toEqual(edit);
    expect(mergeRegister(edit, seen)).toEqual(edit);
  });

  it("a device 1 h fast cannot dominate after its first offset", () => {
    const base = { ms: SERVER };
    const fast = device("fast", HOUR, sharedStorage(), base).clock;
    const before = fast.stamp();
    expect(parseStamp(before)?.ms).toBe(SERVER + HOUR);
    fast.onServerTime(SERVER);
    const after = parseStamp(fast.stamp());
    expect(after?.ms).toBeLessThanOrEqual(SERVER + 60_000);

    base.ms += 1000;
    const honest = device("honest", 0, sharedStorage(), base).clock;
    fast.onServerTime(base.ms);
    const later = fast.stamp();
    base.ms += 61_000;
    const newer = honest.stamp();
    expect(compareStamps(newer, later)).toBeGreaterThan(0);
  });

  it("a clock set a year ahead is pulled back after any sync", () => {
    const fast = device("fast", 365 * 24 * HOUR).clock;
    fast.stamp();
    fast.onServerTime(SERVER);
    expect(parseStamp(fast.stamp())?.ms).toBe(SERVER + 60_000);
  });

  it("stamps keep rising when the clock moves backwards mid-session", () => {
    const base = { ms: SERVER };
    const { clock } = device("d", 0, sharedStorage(), base);
    const first = clock.stamp();
    base.ms -= HOUR;
    const second = clock.stamp();
    base.ms -= HOUR;
    const third = clock.stamp();
    expect(compareStamps(second, first)).toBeGreaterThan(0);
    expect(compareStamps(third, second)).toBeGreaterThan(0);
  });

  it("two tabs stamping at once never repeat and stay ordered", () => {
    const storage = sharedStorage();
    const base = { ms: SERVER };
    const tabA = device("d", 0, storage, base).clock;
    const tabB = device("d", 0, storage, base).clock;
    const stamps = [tabA.stamp(), tabB.stamp(), tabA.stamp(), tabB.stamp()];
    expect(new Set(stamps).size).toBe(4);
    expect(stamps).toEqual([...stamps].sort());
  });

  it("same-ms writes count up, and overflow bumps the ms", () => {
    const storage = sharedStorage();
    storage.write({ offset: 0, lastMs: SERVER, lastCounter: 0xffff });
    const { clock } = device("d", 0, storage);
    const parsed = parseStamp(clock.stamp());
    expect(parsed).toEqual({ ms: SERVER + 1, counter: 0, device: "d" });
  });

  it("a wrong clock with no offset still writes, and may lose, and converges", () => {
    const slow = device("slow", -HOUR).clock;
    const local = { stamp: slow.stamp(), value: "offline, clock wrong" };
    const server = { stamp: `${SERVER}-0000-other`, value: "newer elsewhere" };
    expect(mergeRegister(local, server)).toEqual(server);
    expect(mergeRegister(server, local)).toEqual(server);
  });

  it("offset refreshes on the next sync, as after sleep or wake", () => {
    const base = { ms: SERVER };
    const { clock } = device("d", -HOUR, sharedStorage(), base);
    expect(clock.now()).toBe(SERVER - HOUR);
    clock.onServerTime(SERVER);
    expect(clock.now()).toBe(SERVER);
  });

  it("a signed-out edit folds in with its own time", () => {
    const { clock } = device("d", 0);
    expect(parseStamp(clock.stampAt(SERVER - HOUR))?.ms).toBe(SERVER - HOUR);
  });
});
