import { asNumber, asObject, readJSON, writeJSON } from "#lib/storage/index.js";

import { compareStamps, formatStamp, parseStamp } from "./merge";

/**
 * Hybrid logical clock for one account on one install. `stamp()` is synchronous
 * because reader writes are, so the state lives in `localStorage`, and tabs share
 * it by reading before each stamp. Merging a received stamp advances the clock,
 * so an edit made after seeing a row always beats that row, whatever the device
 * clock says. Device clocks only ever set a floor, never the order of a merge.
 */

/** A clock once set ahead cannot poison later stamps past this. */
const MAX_AHEAD_MS = 60_000;
const COUNTER_MAX = 0xffff;

export interface ClockState {
  readonly offset: number;
  readonly lastMs: number;
  readonly lastCounter: number;
}

export interface ClockStorage {
  read(): ClockState | undefined;
  write(state: ClockState): void;
}

export interface SyncClock {
  /** Next stamp: always above every stamp this account has issued or seen here. */
  stamp(): string;
  /** A stamp for an edit made signed out, which folds in at sign-in with its own time. */
  stampAt(ms: number): string;
  /** Advance past a stamp received from the server or another tab. */
  observe(stamp: string | null): void;
  /** Learn the server's time from any sync response. */
  onServerTime(serverMs: number): void;
  /** Local time corrected by the last known offset. Use for staleness, labels and local days. */
  now(): number;
}

const EMPTY: ClockState = { offset: 0, lastMs: 0, lastCounter: 0 };

export function createClock(
  deviceId: string,
  storage: ClockStorage,
  localNow: () => number = () => Date.now(),
): SyncClock {
  const load = (): ClockState => storage.read() ?? EMPTY;

  const next = (floorMs: number): string => {
    const state = load();
    let ms = Math.max(floorMs, state.lastMs);
    let counter = ms === state.lastMs ? state.lastCounter + 1 : 0;
    if (counter > COUNTER_MAX) {
      ms += 1;
      counter = 0;
    }
    // Persist before use: a stamp must never repeat after a reload.
    storage.write({ ...state, lastMs: ms, lastCounter: counter });
    return formatStamp(ms, counter, deviceId);
  };

  return {
    stamp: () => next(localNow() + load().offset),
    stampAt: (ms) => {
      // Older than the clock's floor on purpose: an offline edit keeps its own time.
      const state = load();
      const stamp = formatStamp(Math.max(0, ms), 0, deviceId);
      const seen = formatStamp(state.lastMs, state.lastCounter, deviceId);
      if (compareStamps(stamp, seen) > 0) {
        storage.write({ ...state, lastMs: Math.max(0, ms), lastCounter: 0 });
      }
      return stamp;
    },
    observe(stamp) {
      const parsed = stamp === null ? null : parseStamp(stamp);
      if (parsed === null) return;
      const state = load();
      const newer =
        parsed.ms > state.lastMs ||
        (parsed.ms === state.lastMs && parsed.counter > state.lastCounter);
      if (newer) storage.write({ ...state, lastMs: parsed.ms, lastCounter: parsed.counter });
    },
    onServerTime(serverMs) {
      const state = load();
      const offset = serverMs - localNow();
      const ceiling = serverMs + MAX_AHEAD_MS;
      if (state.lastMs > ceiling) {
        storage.write({ offset, lastMs: ceiling, lastCounter: 0 });
      } else {
        storage.write({ ...state, offset });
      }
    },
    now: () => localNow() + load().offset,
  };
}

function clockKey(owner: string): string {
  return `easyquran.sync.clock.${owner}`;
}

function decodeClockState(raw: ReturnType<typeof readJSON>): ClockState | undefined {
  const obj = asObject(raw);
  if (obj === undefined || obj === null) return undefined;
  const bound = Number.MAX_SAFE_INTEGER;
  const offset = asNumber(obj.offset, -bound, bound);
  const lastMs = asNumber(obj.lastMs, 0, bound);
  const lastCounter = asNumber(obj.lastCounter, 0, COUNTER_MAX);
  if (offset === undefined || lastMs === undefined || lastCounter === undefined) return undefined;
  return { offset, lastMs, lastCounter };
}

/** Local storage for this account; the in-memory copy covers a blocked or full `localStorage`. */
export function localClockStorage(owner: string): ClockStorage {
  let memory: ClockState | undefined;
  return {
    read() {
      const stored = decodeClockState(readJSON(clockKey(owner)));
      if (stored === undefined) return memory;
      if (memory === undefined) return stored;
      return memory.lastMs > stored.lastMs ? { ...stored, ...memory } : stored;
    },
    write(state) {
      memory = state;
      writeJSON(clockKey(owner), state);
    },
  };
}
