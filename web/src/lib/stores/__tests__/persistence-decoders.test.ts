import { describe, it, expect, vi } from "vite-plus/test";

vi.mock("$env/dynamic/public", () => ({ env: {} }));

import { decodeConsent } from "../consent.svelte";
import { decodeFcm } from "../notifications.svelte";

describe("decodeConsent", () => {
  it("analytics/performance default ON unless explicitly false; advertising OFF unless true", () => {
    expect(decodeConsent({})).toEqual({ analytics: true, performance: true, advertising: false });
    expect(decodeConsent({ analytics: false })).toEqual({
      analytics: false,
      performance: true,
      advertising: false,
    });
    expect(decodeConsent({ advertising: true })).toEqual({
      analytics: true,
      performance: true,
      advertising: true,
    });
    expect(decodeConsent({ analytics: "false", advertising: 1 })).toEqual({
      analytics: true,
      performance: true,
      advertising: false,
    });
  });
  it("falls back to defaults for non-objects", () => {
    expect(decodeConsent(null)).toEqual({ analytics: true, performance: true, advertising: false });
    expect(decodeConsent("x")).toEqual({ analytics: true, performance: true, advertising: false });
  });
});

describe("decodeFcm", () => {
  it("keeps a string token and boolean subscribed", () => {
    expect(decodeFcm({ token: "abc", subscribed: true })).toEqual({
      token: "abc",
      subscribed: true,
    });
  });
  it("nulls non-string tokens and only accepts subscribed === true", () => {
    expect(decodeFcm({ token: 5, subscribed: 1 })).toEqual({ token: null, subscribed: false });
    expect(decodeFcm({ token: null, subscribed: "true" })).toEqual({
      token: null,
      subscribed: false,
    });
  });
  it("defaults when absent or non-object", () => {
    expect(decodeFcm({})).toEqual({ token: null, subscribed: false });
    expect(decodeFcm(undefined)).toEqual({ token: null, subscribed: false });
  });
});
