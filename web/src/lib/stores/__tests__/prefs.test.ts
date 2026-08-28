import { describe, it, expect, beforeEach, vi } from "vite-plus/test";

vi.mock("$app/environment", () => ({ browser: true, dev: false }));
vi.mock("$env/dynamic/public", () => ({ env: {} }));

import { prefs } from "../prefs.svelte";

const KEY = "easyquran.prefs";

// happy-dom reports `prefers-color-scheme: dark` as false, so a "system" appearance resolves
// to light inside these tests — asserted explicitly below so the resolution stays documented.
describe("prefs cross-tab wiring", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-palette");
    document.documentElement.removeAttribute("data-mode");
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("data-surface");
    document.documentElement.removeAttribute("data-accent");
    prefs.hydrate();
  });

  it("defaults to the sacred palette with system appearance", () => {
    expect(prefs.palette).toBe("sacred");
    expect(prefs.mode).toBe("system");
    expect(prefs.theme).toBe("light");
    expect(document.documentElement.dataset.palette).toBe(undefined);
  });

  it("re-applies a foreign tab's prefs from a storage event", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "light", surface: "paper", accent: "gold", instantResume: true }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(prefs.mode).toBe("light");
    expect(prefs.palette).toBe("sepia");
    expect(prefs.instantResume).toBe(true);
    expect(document.documentElement.dataset.palette).toBe("sepia");
    expect(document.documentElement.dataset.mode).toBe("light");
    // Retired attribute names must not come back.
    expect(document.documentElement.dataset.theme).toBe(undefined);
    expect(document.documentElement.dataset.surface).toBe(undefined);
    expect(document.documentElement.dataset.accent).toBe(undefined);
  });

  it("migrates legacy surface and theme values onto palette/mode", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ theme: "dark", surface: "mocha" }));
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(prefs.palette).toBe("sacred");
    expect(prefs.mode).toBe("dark");
    expect(document.documentElement.dataset.palette).toBe("sacred");
    expect(document.documentElement.dataset.mode).toBe("dark");
  });

  it("prefers explicit new fields over migrated legacy ones", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ palette: "sapphire", mode: "dark", surface: "paper", theme: "light" }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(prefs.palette).toBe("sapphire");
    expect(prefs.mode).toBe("dark");
    // Legacy surface mirrors the palette for the frozen settings-document decoder.
    expect(prefs.surface).toBe("slate");
    expect(document.documentElement.dataset.palette).toBe("sapphire");
    expect(document.documentElement.dataset.mode).toBe("dark");
  });

  it("drops invalid fields from a foreign tab instead of applying them", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "blue", surface: "nope", accent: 7, mode: "sepia", custom: { bg: "red" } }),
    );
    window.dispatchEvent(new StorageEvent("storage", { key: KEY }));
    expect(prefs.palette).toBe("sacred");
    expect(prefs.mode).toBe("system");
    expect(prefs.accent).toBe("emerald");
    expect(prefs.hasCustom).toBe(false);
  });

  it("ignores storage events for other keys", () => {
    window.localStorage.setItem("easyquran.reader", JSON.stringify({ v: 3 }));
    window.dispatchEvent(new StorageEvent("storage", { key: "easyquran.reader" }));
    expect(prefs.palette).toBe("sacred");
    expect(document.documentElement.dataset.palette).toBe(undefined);
  });
});

describe("prefs setters", () => {
  beforeEach(() => {
    window.localStorage.clear();
    prefs.reset();
  });

  it("setPalette applies attributes and syncs the legacy surface field", () => {
    prefs.setPalette("ink");
    expect(prefs.palette).toBe("ink");
    expect(prefs.surface).toBe("ink");
    expect(document.documentElement.dataset.palette).toBe("ink");
    const stored = JSON.parse(window.localStorage.getItem(KEY) ?? "{}");
    expect(stored.palette).toBe("ink");
    expect(stored.surface).toBe("ink");
  });

  it("setMode resolves system and persists the explicit choice", () => {
    prefs.setMode("dark");
    expect(prefs.mode).toBe("dark");
    expect(document.documentElement.dataset.mode).toBe("dark");
    prefs.setMode("system");
    expect(prefs.mode).toBe("system");
    // happy-dom resolves prefers-color-scheme: dark to false → light.
    expect(document.documentElement.dataset.mode).toBe("light");
    expect(prefs.theme).toBe("light");
  });

  it("toggleTheme flips the resolved mode", () => {
    prefs.setMode("dark");
    prefs.toggleTheme();
    expect(prefs.theme).toBe("light");
    expect(document.documentElement.dataset.mode).toBe("light");
  });
});
