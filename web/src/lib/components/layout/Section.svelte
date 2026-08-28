<script lang="ts">
  import { cn } from "$lib/utils";
  import Band from "./Band.svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

  /**
   * DEPRECATED (plan 04): the contained-section model the band model replaces. Kept
   * rendering-identical, reimplemented over `Band` (tone="page" paints nothing,
   * pad="none" + its own legacy padding, contentClass restores the old fixed px-6
   * gutter across the whole §15 ladder), so no existing call site moves. New surfaces
   * use `Band` directly.
   */
  let {
    width = "default",
    border = true,
    tight = false,
    class: className = "",
    children,
    ...rest
  }: {
    width?: "default" | "narrow" | "wide";
    border?: boolean;
    tight?: boolean;
    class?: string;
    children: Snippet;
  } & HTMLAttributes<HTMLElement> = $props();
</script>

<Band
  tone="page"
  pad="none"
  {width}
  rule={border}
  class={cn(tight ? "py-12" : "py-16 md:py-24", className)}
  contentClass="px-6 md:px-6 lg:px-6 xl:px-6"
  {...rest}
>
  {@render children()}
</Band>
