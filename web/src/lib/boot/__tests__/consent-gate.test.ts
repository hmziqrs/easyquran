import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { consent } from "#lib/stores/consent.svelte.js";

import { startAnalytics } from "../analytics";
import { startCrashReporting } from "../crash-reporting";

vi.mock("$app/env", () => ({ browser: true }));

const loaded = vi.hoisted(() => ({ count: 0, exceptions: 0 }));

vi.mock("#lib/firebase/analytics.js", () => {
  loaded.count += 1;
  return {
    initAnalytics: async () => undefined,
    setConsentState: async () => undefined,
    setAnalyticsCollectionEnabled: async () => undefined,
    pageView: async () => undefined,
    logException: async () => {
      loaded.exceptions += 1;
    },
  };
});

describe("consent gate", () => {
  beforeEach(() => {
    consent.set({ analytics: false });
  });

  it("loads nothing until consent turns on", async () => {
    const stop = startAnalytics();
    const stopCrash = startCrashReporting();
    window.dispatchEvent(new ErrorEvent("error", { message: "boom" }));
    await vi.dynamicImportSettled();
    expect(loaded.count).toBe(0);
    expect(loaded.exceptions).toBe(0);

    consent.set({ analytics: true });
    await vi.dynamicImportSettled();
    expect(loaded.count).toBe(1);

    window.dispatchEvent(new ErrorEvent("error", { message: "boom" }));
    await vi.dynamicImportSettled();
    expect(loaded.exceptions).toBe(1);
    stop();
    stopCrash();
  });
});
