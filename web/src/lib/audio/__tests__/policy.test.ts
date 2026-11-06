import { describe, expect, it } from "vite-plus/test";

import {
  OFFLINE_AUDIO_MAX_BYTES,
  OFFLINE_AUDIO_MAX_FILE_BYTES,
  PLAYBACK_RATES,
  REPEAT_LIMITS,
  admitOfflineBytes,
  clampInt,
  nearestRate,
  retryDelayMs,
} from "../policy";

describe("nearestRate", () => {
  it("snaps to the nearest allowed rate", () => {
    expect(nearestRate(1.4)).toBe(1.25);
    expect(nearestRate(0.1)).toBe(0.75);
    expect(nearestRate(9)).toBe(2);
  });

  it("falls back to 1x for non-finite input", () => {
    expect(nearestRate(Number.NaN)).toBe(1);
    expect(PLAYBACK_RATES).toContain(nearestRate(1.5));
  });
});

describe("retryDelayMs", () => {
  it("backs off 1 s, 2 s, 4 s then gives up", () => {
    expect(retryDelayMs(1)).toBe(1000);
    expect(retryDelayMs(2)).toBe(2000);
    expect(retryDelayMs(3)).toBe(4000);
    expect(retryDelayMs(4)).toBeNull();
  });

  it("rejects attempt 0 and fractions", () => {
    expect(retryDelayMs(0)).toBeNull();
    expect(retryDelayMs(1.5)).toBeNull();
  });
});

describe("admitOfflineBytes", () => {
  it("admits a file that fits the device budget", () => {
    expect(admitOfflineBytes(0, 1024)).toBe(true);
    expect(admitOfflineBytes(OFFLINE_AUDIO_MAX_BYTES - 1024, 1024)).toBe(true);
  });

  it("refuses a file that would pass the device budget", () => {
    expect(admitOfflineBytes(OFFLINE_AUDIO_MAX_BYTES, 1)).toBe(false);
  });

  it("refuses a single file over the per-file cap", () => {
    expect(admitOfflineBytes(0, OFFLINE_AUDIO_MAX_FILE_BYTES + 1)).toBe(false);
  });

  it("refuses empty or non-finite sizes", () => {
    expect(admitOfflineBytes(0, 0)).toBe(false);
    expect(admitOfflineBytes(0, Number.NaN)).toBe(false);
  });
});

describe("clampInt", () => {
  it("clamps repeat counts into their limits", () => {
    expect(clampInt(0, REPEAT_LIMITS.ayahRepeat)).toBe(1);
    expect(clampInt(99, REPEAT_LIMITS.ayahRepeat)).toBe(20);
    expect(clampInt(2.6, REPEAT_LIMITS.setRepeat)).toBe(3);
  });

  it("uses the minimum for non-finite input", () => {
    expect(clampInt(Number.NaN, REPEAT_LIMITS.delayMs)).toBe(0);
  });
});
