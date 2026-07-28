<!--
  Panel — full-bleed accent block. Variants: accent (solid brand), soft
  (tinted), ink (inverted). CTA sections compose content inside it.
-->
<script lang="ts">
  import { cn } from "$lib/utils";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

  let {
    variant = "accent",
    class: className = "",
    children,
    ...rest
  }: {
    variant?: "accent" | "soft" | "ink";
    class?: string;
    children: Snippet;
  } & HTMLAttributes<HTMLDivElement> = $props();

  const variants = {
    accent: "bg-accent text-accent-fg",
    soft: "bg-accent-soft text-fg border border-accent-line",
    ink: "bg-fg text-bg",
  } as const;
</script>

<div
  class={cn(
    "relative overflow-hidden rounded-2xl p-8 md:p-10 lg:p-12",
    variants[variant],
    className,
  )}
  {...rest}
>
  {@render children()}
</div>
