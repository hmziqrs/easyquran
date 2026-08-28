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

<!-- Pill input inside the 10px block (plan 06: the command palette is a block, its
     input is a control, so it takes the control pill). -->
<div data-slot="command-input-wrapper" class="border-b border-border p-3">
  <div
    class="flex h-11 items-center gap-2.5 rounded-pill border border-border bg-background-subtle px-4 focus-within:border-border-strong"
  >
    <Icon name="search" size={16} class="shrink-0 text-muted" />
    <CommandPrimitive.Input
      bind:ref
      bind:value
      data-slot="command-input"
      class={cn(
        "w-full bg-transparent text-body text-foreground outline-none placeholder:text-muted disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...restProps}
    />
  </div>
</div>
