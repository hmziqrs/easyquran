<script lang="ts">
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { loadPalette } from "./palette-loader";
  import { Icon } from "$lib/components/icon";
  import { cn } from "$lib/utils";

  let {
    class: className,
    label = "Search",
    inert: isInert = false,
  }: { class?: string; label?: string; inert?: boolean } = $props();
</script>

<button
  type="button"
  onclick={() => commandPalette.show()}
  onpointerenter={() => void loadPalette()}
  onfocus={() => void loadPalette()}
  aria-label={label}
  aria-keyshortcuts="Meta+K Control+K"
  title={`${label} (⌘K)`}
  inert={isInert || undefined}
  aria-hidden={isInert || undefined}
  class={cn(
    "inline-flex h-[38px] w-[38px] items-center justify-center rounded-pill border border-border-strong text-foreground-secondary transition-colors duration-150 hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring",
    className,
  )}
>
  <Icon name="search" size={18} title={label} />
</button>
