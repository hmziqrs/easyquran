<!--
  Card — the raised surface box (bg-1, hairline border, lg radius).
  `interactive` adds the hover border lift used on linked cards.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

  let {
    interactive = false,
    class: className = "",
    children,
    ...rest
  }: {
    interactive?: boolean;
    class?: string;
    children: Snippet;
  } & HTMLAttributes<HTMLDivElement> = $props();
</script>

<div
  class={cn(
    "rounded-lg border border-line bg-bg-1 p-6 transition-colors duration-200",
    interactive && "hover:border-line-2",
    className,
  )}
  {...rest}
>
  {@render children()}
</div>
