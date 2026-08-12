<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn } from "$lib/utils.js";
  import { Icon } from "$lib/components/icon";

  let {
    ref = $bindable(null),
    value = $bindable(""),
    class: className,
    ...restProps
  }: CommandPrimitive.InputProps = $props();
</script>

<div data-slot="command-input-wrapper" class="flex items-center gap-2.5 border-b border-line px-4">
  <Icon name="search" size={16} class="shrink-0 text-fg-3" />
  <CommandPrimitive.Input
    bind:ref
    bind:value
    data-slot="command-input"
    class={cn(
      "h-12 w-full bg-transparent text-[15px] text-fg outline-none placeholder:text-fg-4 disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...restProps}
  />
</div>
