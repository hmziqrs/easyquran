<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: CommandPrimitive.LinkItemProps = $props();
</script>

<CommandPrimitive.LinkItem
  bind:ref
  data-slot="command-link-item"
  class={cn(
    "flex cursor-default select-none items-center gap-3 rounded-lg px-2.5 py-2 text-sm text-fg-2 no-underline outline-none data-selected:bg-bg-2 data-selected:text-fg data-disabled:pointer-events-none data-disabled:opacity-50",
    className,
  )}
  {...restProps}
/>
