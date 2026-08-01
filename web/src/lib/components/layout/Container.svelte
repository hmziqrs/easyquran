<script lang="ts">
  import { cn } from "$lib/utils";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

  let {
    width = "default",
    class: className = "",
    children,
    ...rest
  }: {
    width?: "default" | "narrow" | "wide";
    class?: string;
    children: Snippet;
  } & HTMLAttributes<HTMLDivElement> = $props();

  const max = $derived(
    { default: "max-w-[1200px]", narrow: "max-w-[880px]", wide: "max-w-[1440px]" }[width],
  );
</script>

<div class={cn("mx-auto w-full px-6", max, className)} {...rest}>{@render children()}</div>
