<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import { cn, type WithElementRef } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLSpanElement>, HTMLSpanElement> = $props();
</script>

<span
  bind:this={ref}
  data-slot="command-shortcut"
  class={cn(
    "ml-auto shrink-0 rounded border border-line-2 px-1.5 py-0.5 font-mono text-[10.5px] tracking-widest text-fg-4",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</span>
