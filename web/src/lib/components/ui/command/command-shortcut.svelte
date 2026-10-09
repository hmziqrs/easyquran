<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import { cn, type WithElementRef } from "#lib/utils.js";

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
    "ms-auto shrink-0 rounded border border-line-2 px-1.5 py-0.5 text-[11px] font-medium text-fg-4",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</span>
