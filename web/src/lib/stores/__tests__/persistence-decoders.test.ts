import { describe, it, expect, vi } from "vite-plus/test";

// The decoders under test are pure, but their owning store modules transitively
// import `$env/dynamic/public` (config/site via prefs; firebase/index via
// notifications). That SvelteKit virtual module does not initialize under the
// vitest module runner, so stub it to an empty env — the decoders use none of
// its values.
vi.mock("$env/dynamic/public", () => ({ env: {} }));

import { decodeConsent } from "../consent.svelte";
import { decodeFcm } from "../notifications.svelte";
import { decodePrefs } from "../prefs.svelte";

describe("decodePrefs", () => {
  it("keeps valid theme/accent", () => {
    expect(decodePrefs({ theme: "light", accent: "gold" })).toEqual({
      theme: "light",
      accent: "gold",
    });
  });
  it("falls back to defaults for invalid/missing values", () => {
    expect(decodePrefs({ theme: "mauve", accent: "nope" })).toEqual({
      theme: "dark",
      accent: "emerald",
    });
    expect(decodePrefs({})).toEqual({ theme: "dark", accent: "emerald" });
    expect(decodePrefs(null)).toEqual({ theme: "dark", accent: "emerald" });
  });
});

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
    // non-boolean values keep defaults
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
