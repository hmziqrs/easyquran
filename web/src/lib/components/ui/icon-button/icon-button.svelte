<script lang="ts">
  import { cn, externalLinkAttrs, type WithElementRef } from "$lib/utils";
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  import {
    iconButtonVariants,
    type IconButtonVariant,
    type IconButtonSize,
  } from "./icon-button-variants";

  type Props = WithElementRef<HTMLButtonAttributes & HTMLAnchorAttributes> & {
    /** Accessible name (§51): icon-only controls must have a label; rendered as aria-label. */
    label: string;
    variant?: IconButtonVariant;
    size?: IconButtonSize;
    href?: string;
    type?: HTMLButtonAttributes["type"];
    class?: string;
    children?: Snippet;
  };

  let {
    ref = $bindable<HTMLElement | null>(null),
    label,
    variant = "ghost",
    size = "md",
    href,
    type = "button",
    class: className,
    children,
    ...rest
  }: Props = $props();
</script>

{#if href}
  <a
    bind:this={ref}
    aria-label={label}
    class={cn(iconButtonVariants({ variant, size }), className)}
    {href}
    {...externalLinkAttrs(href)}
    {...(rest as HTMLAnchorAttributes)}
  >
    {@render children?.()}
  </a>
{:else}
  <button
    bind:this={ref}
    aria-label={label}
    class={cn(iconButtonVariants({ variant, size }), className)}
    {type}
    {...(rest as HTMLButtonAttributes)}
  >
    {@render children?.()}
  </button>
{/if}
