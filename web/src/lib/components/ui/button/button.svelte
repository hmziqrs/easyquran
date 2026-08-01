<script lang="ts">
  import { cn, externalLinkAttrs, type WithElementRef } from "$lib/utils";
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  import { buttonVariants, type ButtonVariant, type ButtonSize } from "./button-variants";

  type Props = WithElementRef<HTMLButtonAttributes & HTMLAnchorAttributes> & {
    variant?: ButtonVariant;
    size?: ButtonSize;
    href?: string;
    class?: string;
    arrow?: boolean;
    children?: Snippet;
  };

  let {
    ref = $bindable<HTMLElement | null>(null),
    variant = "primary",
    size = "md",
    href,
    class: className,
    arrow = false,
    children,
    ...rest
  }: Props = $props();
</script>

{#if href}
  <a
    bind:this={ref}
    class={cn(buttonVariants({ variant, size }), className)}
    {href}
    {...externalLinkAttrs(href)}
    {...(rest as HTMLAnchorAttributes)}
  >
    {@render children?.()}
    {#if arrow}<span
        class="transition-transform duration-150 ease-out group-hover:translate-x-0.5">→</span
      >{/if}
  </a>
{:else}
  <button
    bind:this={ref}
    class={cn(buttonVariants({ variant, size }), className)}
    {...(rest as HTMLButtonAttributes)}
  >
    {@render children?.()}
    {#if arrow}<span
        class="transition-transform duration-150 ease-out group-hover:translate-x-0.5">→</span
      >{/if}
  </button>
{/if}
