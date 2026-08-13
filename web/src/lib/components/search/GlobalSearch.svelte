<script lang="ts">
  /**
   * Always-mounted, deliberately tiny: it owns only the global shortcuts and
   * fetches the real palette on first use. Keep this file free of imports from
   * `$lib/search/palette`, `bits-ui`, the Quran stores, AND `@tanstack/hotkeys`
   * — anything static here lands in the initial bundle on every page. The
   * hotkeys dependency is dynamically imported below so it lives in its own
   * chunk instead of weighing down first paint.
   */
  import type { Component } from "svelte";
  import { page } from "$app/state";
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { loadPalette } from "./palette-loader";
  import type { HotkeyHandle } from "$lib/hotkeys.svelte";

  let Palette = $state<Component | null>(null);
  let slashHandle: HotkeyHandle | undefined;

  async function ensureLoaded(): Promise<void> {
    if (Palette) return;
    Palette = await loadPalette();
  }

  function open(): void {
    void ensureLoaded();
    commandPalette.show();
  }

  // Register the global chords asynchronously so `@tanstack/hotkeys` (and the
  // HotkeyManager singleton) stay out of the initial bundle. There is a brief
  // window after load before the chunk resolves and shortcuts go live — fine
  // for a command palette. Mod+K toggles everywhere (ignoreInputs is false for
  // Mod chords by default, so it fires from inside an input); `/` only opens
  // when the palette is closed and focus is outside editable elements
  // (ignoreInputs defaults true for single keys). Its `enabled` is synced to
  // `commandPalette.open` below so the query may contain `/`.
  $effect(() => {
    let destroyed = false;
    let cleanup: (() => void) | undefined;

    void import("$lib/hotkeys.svelte").then(({ registerHotkey }) => {
      if (destroyed) return;
      const modK = registerHotkey("Mod+K", () => {
        if (commandPalette.open) commandPalette.hide();
        else open();
      }, { meta: { name: "Toggle command palette" } });

      const slash = registerHotkey("/", (event) => {
        // IME: don't let a composition session's "/" hijack the page.
        if (event.isComposing) return;
        open();
      }, { meta: { name: "Open command palette" } });
      slashHandle = slash;

      const resume = registerHotkey("`", (event) => {
        if (event.isComposing) return;
        void Promise.all([import("$lib/reader/resume"), import("$lib/data/quran")]).then(
          ([{ resumeToLastRead }, { routeContextFromParams }]) => {
            void resumeToLastRead(routeContextFromParams(page.params));
          },
        );
      }, { meta: { name: "Continue reading" } });

      cleanup = () => {
        modK.unregister();
        slash.unregister();
        resume.unregister();
      };
    });

    return () => {
      destroyed = true;
      cleanup?.();
      slashHandle = undefined;
    };
  });

  // Disable `/` while the palette is already open so the query can contain it.
  $effect(() => {
    const open = commandPalette.open;
    slashHandle?.setOptions({ enabled: !open });
  });

  // Covers every other way the palette can be opened — the nav trigger today,
  // any future caller of `commandPalette.show()` tomorrow.
  $effect(() => {
    if (commandPalette.open) void ensureLoaded();
  });
</script>

{#if Palette}
  <Palette />
{/if}
