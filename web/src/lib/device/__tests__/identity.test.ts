import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { isUuidV7 } from "#lib/sync/ids.js";

import { deviceIdFor } from "../identity";

vi.mock("$app/env", () => ({ browser: true }));

describe("device id", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("is stable per account, and accounts share nothing", () => {
    const first = deviceIdFor("7");
    expect(isUuidV7(first)).toBe(true);
    expect(deviceIdFor("7")).toBe(first);
    expect(deviceIdFor("9")).not.toBe(first);
  });
});
