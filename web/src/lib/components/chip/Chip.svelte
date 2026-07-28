<!--
  Chip — small pill label. Variants: default / accent / ghost. Optional dot.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  let {
    dot = false,
    accent = false,
    ghost = false,
    class: className = "",
    children,
  }: {
    dot?: boolean;
    accent?: boolean;
    ghost?: boolean;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<span
  class={cn(
    "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 font-mono text-xs text-fg-2",
    ghost ? "bg-transparent" : "bg-bg-1",
    accent ? "border-accent-line bg-accent-soft text-accent" : "border-line-2",
    className,
  )}
>
  {#if dot}<span class="size-1.5 rounded-full bg-accent"></span>{/if}
  {@render children()}
</span>
