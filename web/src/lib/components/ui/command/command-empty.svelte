<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: CommandPrimitive.EmptyProps = $props();
</script>

<CommandPrimitive.Empty
  bind:ref
  data-slot="command-empty"
  class={cn("px-4 py-10 text-center text-sm text-fg-3", className)}
  {...restProps}
/>
