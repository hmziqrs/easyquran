/**
 * Shared keyboard-shortcut registry backed by TanStack Hotkeys
 * (`@tanstack/hotkeys`, framework-agnostic core). Every surface registers its
 * chords through `registerHotkey` so they share one HotkeyManager singleton:
 * devtools, conflict detection, input filtering, and platform-aware `Mod`
 * resolution (Cmd on macOS, Ctrl elsewhere) come for free.
 *
 * The manager attaches to `document`, so call `registerHotkey` only from a
 * client-only context (a Svelte `$effect` runs in the browser, never during
 * SSR). The returned handle must be `unregister()`-ed on cleanup.
 */
import {
  getHotkeyManager,
  type Hotkey,
  type HotkeyCallback,
  type HotkeyOptions,
} from "@tanstack/hotkeys";

export type { Hotkey, HotkeyCallback, HotkeyOptions };
export type HotkeyHandle = ReturnType<ReturnType<typeof getHotkeyManager>["register"]>;

export function registerHotkey(
  hotkey: Hotkey,
  callback: HotkeyCallback,
  options?: HotkeyOptions,
): HotkeyHandle {
  return getHotkeyManager().register(hotkey, callback, options);
}
