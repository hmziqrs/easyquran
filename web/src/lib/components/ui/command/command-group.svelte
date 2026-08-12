<script lang="ts">
  import { Command as CommandPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$lib/utils.js";
  import type { Snippet } from "svelte";
  import type { ComponentProps } from "svelte";

  let {
    ref = $bindable(null),
    class: className,
    heading,
    headingProps,
    children,
    ...restProps
  }: WithoutChildrenOrChild<CommandPrimitive.GroupProps> & {
    heading?: string;
    headingProps?: WithoutChildrenOrChild<ComponentProps<typeof CommandPrimitive.GroupHeading>>;
    children: Snippet;
  } = $props();
</script>

<CommandPrimitive.Group
  bind:ref
  data-slot="command-group"
  class={cn("overflow-hidden p-1.5", className)}
  {...restProps}
>
  {#if heading}
    <CommandPrimitive.GroupHeading
      data-slot="command-group-heading"
      {...headingProps}
      class={cn(
        "px-2.5 pb-1 pt-1.5 text-[11px] font-semibold uppercase tracking-[0.08em] text-fg-4",
        headingProps?.class,
      )}
    >
      {heading}
    </CommandPrimitive.GroupHeading>
  {/if}
  <CommandPrimitive.GroupItems data-slot="command-group-items">
    {@render children()}
  </CommandPrimitive.GroupItems>
</CommandPrimitive.Group>
