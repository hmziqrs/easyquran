<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: CommandPrimitive.ItemProps = $props();
</script>

<CommandPrimitive.Item
  bind:ref
  data-slot="command-item"
  class={cn(
    "flex cursor-default select-none items-center gap-3 rounded-md px-2.5 py-2 text-sm text-foreground-secondary outline-none data-selected:bg-surface-hover data-selected:text-foreground data-disabled:pointer-events-none data-disabled:opacity-50",
    className,
  )}
  {...restProps}
/>
