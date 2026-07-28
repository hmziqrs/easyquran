<!--
  Section — vertical page section with an optional top divider. Content is
  auto-wrapped in a Container so it's never full-bleed.
  • default padding: 64px (mobile) / 96px (desktop)
  • `tight`: 48px — for denser stacked sections
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import Container from "./Container.svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

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

<section
  class={cn(tight ? "py-12" : "py-16 md:py-24", border && "border-t border-line", className)}
  {...rest}
>
  <Container {width}>{@render children()}</Container>
</section>
