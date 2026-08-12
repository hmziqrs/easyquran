<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn } from "$lib/utils.js";

  let {
    ref = $bindable(null),
    class: className,
    ...restProps
  }: CommandPrimitive.LoadingProps = $props();
</script>

<CommandPrimitive.Loading
  bind:ref
  data-slot="command-loading"
  class={cn("px-4 py-3 text-xs text-fg-3", className)}
  {...restProps}
/>
