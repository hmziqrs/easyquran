<script lang="ts">
  import { commandPalette } from "$lib/stores/command-palette.svelte";
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
  aria-label={label}
  aria-keyshortcuts="Meta+K Control+K"
  title={`${label} (⌘K)`}
  inert={isInert || undefined}
  aria-hidden={isInert || undefined}
  class={cn(
    "inline-flex h-[38px] w-[38px] items-center justify-center rounded-[11px] border border-line-2 text-fg-2 transition-colors duration-150 hover:bg-bg-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
    className,
  )}
>
  <Icon name="search" size={18} title={label} />
</button>
