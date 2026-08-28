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
    "rounded-lg border border-border bg-surface p-6 transition-colors duration-200",
    interactive && "hover:border-border-strong",
    className,
  )}
  {...rest}
>
  {@render children()}
</div>
