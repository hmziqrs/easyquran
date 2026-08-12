<script lang="ts">
  /**
   * Always-mounted, deliberately tiny: it owns only the global shortcuts and
   * fetches the real palette on first use. Keep this file free of imports from
   * `$lib/search/palette`, `bits-ui` or the Quran stores — anything static here
   * lands in the initial bundle on every page.
   */
  import type { Component } from "svelte";
  import { commandPalette, isPaletteChord } from "$lib/stores/command-palette.svelte";
  import { loadPalette } from "./palette-loader";

  const EDITABLE = /^(input|textarea|select)$/i;

  let Palette = $state<Component | null>(null);

  async function ensureLoaded(): Promise<void> {
    if (Palette) return;
    Palette = await loadPalette();
  }

  function open(): void {
    void ensureLoaded();
    commandPalette.show();
  }

  function onWindowKeydown(e: KeyboardEvent): void {
    if (isPaletteChord(e)) {
      e.preventDefault();
      if (commandPalette.open) commandPalette.hide();
      else open();
      return;
    }
    // `isComposing` keeps an IME's own "/" from hijacking the page.
    if (e.isComposing) return;
    if (e.key !== "/" || commandPalette.open || e.metaKey || e.ctrlKey || e.altKey) return;
    const target = e.target as HTMLElement | null;
    if (target?.isContentEditable || EDITABLE.test(target?.tagName ?? "")) return;
    e.preventDefault();
    open();
  }

  // Covers every other way the palette can be opened — the nav trigger today,
  // any future caller of `commandPalette.show()` tomorrow.
  $effect(() => {
    if (commandPalette.open) void ensureLoaded();
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if Palette}
  <Palette />
{/if}
