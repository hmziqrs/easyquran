import { describe, expect, it } from "vite-plus/test";

import { handoffPlace, isThreadHere } from "../handoff";
import { decodeThread, type ReadingThread } from "../schema";

const ID = "d5d5d5d5-d5d5-7d5d-8d5d-d5d5d5d5d5a1";

function thread(overrides: Readonly<Record<string, string | number | null>>): ReadingThread {
  const decoded = decodeThread({
    id: ID,
    origin: "index",
    scope: "surah:2",
    surah: 2,
    ayah: 7,
    device: "phone",
    startedAt: "2026-11-01T00:00:00.000Z",
    updatedAt: "2026-11-01T00:00:00.000Z",
    state: "open",
    ...overrides,
  });
  if (decoded === null) throw new Error("fixture thread must decode");
  return decoded;
}

describe("decodeThread keeps the server's device label", () => {
  it("reads deviceName and defaults it to null for an unknown, removed or unnamed device", () => {
    expect(thread({ deviceId: "dev-pixel", deviceName: "Pixel 8" }).deviceName).toBe("Pixel 8");
    expect(thread({ deviceId: "dev-gone", deviceName: null }).deviceName).toBeNull();
    expect(thread({ deviceId: "dev-gone" }).deviceName).toBeNull();
    expect(thread({ deviceName: "" }).deviceName).toBeNull();
  });
});

describe("handoff: two phones are two devices", () => {
  it("is here only when the device ids match, even when both are phones", () => {
    const pixel = thread({ deviceId: "dev-pixel", device: "phone" });
    expect(isThreadHere(pixel, { deviceId: "dev-pixel", kind: "phone" })).toBe(true);
    expect(isThreadHere(pixel, { deviceId: "dev-other-phone", kind: "phone" })).toBe(false);
  });

  it("names the other phone when the server resolved its name", () => {
    const pixel = thread({ deviceId: "dev-pixel", deviceName: "Pixel 8", device: "phone" });
    expect(handoffPlace(pixel, { deviceId: "dev-other-phone", kind: "phone" })).toEqual({
      kind: "named",
      name: "Pixel 8",
    });
  });

  it("falls back to the device kind when no name resolves (a removed device)", () => {
    const gone = thread({ deviceId: "dev-gone", deviceName: null, device: "tablet" });
    expect(handoffPlace(gone, { deviceId: "dev-here", kind: "phone" })).toEqual({
      kind: "device",
      device: "tablet",
    });
  });

  it("shows no place for a thread last moved on this device, whatever its name", () => {
    const here = thread({ deviceId: "dev-here", deviceName: "This laptop", device: "desktop" });
    expect(handoffPlace(here, { deviceId: "dev-here", kind: "desktop" })).toEqual({ kind: "here" });
  });

  it("compares the kind for a row that predates device ids", () => {
    const legacy = thread({ device: "phone" });
    expect(handoffPlace(legacy, { deviceId: "dev-here", kind: "phone" })).toEqual({ kind: "here" });
    expect(handoffPlace(legacy, { deviceId: "dev-here", kind: "desktop" })).toEqual({
      kind: "device",
      device: "phone",
    });
  });
});
