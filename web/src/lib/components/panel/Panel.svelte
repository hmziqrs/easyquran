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

  /* §61 semantic tokens only: accent = primary CTA panel, soft = quiet callout,
     ink = inverted. All resolve per palette × mode via the §4 contract. */
  const variants = {
    accent: "bg-primary text-primary-foreground",
    soft: "bg-primary-soft text-foreground border border-border",
    ink: "bg-foreground text-background",
  } as const;
</script>

<div
  class={cn(
    /* Large panel → --radius-xl 14px (plan 06: sheets and popovers; Panel is the
       biggest callout block). */
    "relative overflow-hidden rounded-xl p-8 md:p-10 lg:p-12",
    variants[variant],
    className,
  )}
  {...rest}
>
  {@render children()}
</div>
