import { matchesKeyboardEvent } from "@tanstack/hotkeys";
import { describe, expect, it } from "vite-plus/test";

// Guards patches/@tanstack__hotkeys@0.8.0.patch: isSingleLetterKey matched any
// Unicode letter (\p{Letter}), so a non-Latin input method produced a localized
// event.key that early-exited before the event.code fallback — and Mod+K never
// fired. If the library upgrades and the patch is dropped, these go red.

describe("matchesKeyboardEvent across non-Latin keyboard layouts", () => {
  // The physical K key under each layout (code stays "KeyK", event.key localizes).
  it.each([
    ["Arabic", "ك"],
    ["Cyrillic", "к"],
    ["Hebrew", "כ"],
    ["Greek", "κ"],
  ])("matches Mod+K when the physical K key yields a %s letter", (_script, key) => {
    const mac = new KeyboardEvent("keydown", { key, code: "KeyK", metaKey: true });
    const win = new KeyboardEvent("keydown", { key, code: "KeyK", ctrlKey: true });
    expect(matchesKeyboardEvent(mac, "Mod+K", "mac")).toBe(true);
    expect(matchesKeyboardEvent(win, "Mod+K", "windows")).toBe(true);
  });

  it("still matches plain Latin Mod+K", () => {
    const ev = new KeyboardEvent("keydown", { key: "k", code: "KeyK", metaKey: true });
    expect(matchesKeyboardEvent(ev, "Mod+K", "mac")).toBe(true);
  });

  it("does not match when a different physical key is pressed", () => {
    const ev = new KeyboardEvent("keydown", { key: "ك", code: "KeyL", metaKey: true });
    expect(matchesKeyboardEvent(ev, "Mod+K", "mac")).toBe(false);
  });

  it("matches the backtick chord when the physical key yields an Arabic letter", () => {
    const ev = new KeyboardEvent("keydown", { key: "ذ", code: "Backquote" });
    expect(matchesKeyboardEvent(ev, "`")).toBe(true);
  });
});
