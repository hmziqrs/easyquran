<script lang="ts">
  import { commandPalette } from "$lib/stores/command-palette.svelte";
  import { loadPalette } from "./palette-loader";
  import { Icon } from "$lib/components/icon";
  import { cn } from "$lib/utils";

  /** `pill` renders the marketing header's wide search affordance (label + ⌘K). */
  let {
    class: className,
    label = "Search",
    inert: isInert = false,
    variant = "icon",
  }: { class?: string; label?: string; inert?: boolean; variant?: "icon" | "pill" } = $props();
</script>

{#if variant === "pill"}
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
      "group flex h-10 min-w-0 flex-grow items-center gap-3 rounded-pill border border-border px-4 text-start transition-colors duration-150 hover:border-border-strong hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring",
      className,
    )}
  >
    <Icon name="search" size={17} class="flex-none text-muted" title={label} />
    <span class="min-w-0 flex-grow truncate text-body text-muted">{label}</span>
    <span
      class="hidden flex-none items-center gap-[3px] rounded-pill bg-background-subtle px-[9px] py-[5px] text-[13px] font-bold text-muted sm:flex"
      aria-hidden="true"
    >
      <span>⌘</span><span>K</span>
    </span>
  </button>
{:else}
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
{/if}
