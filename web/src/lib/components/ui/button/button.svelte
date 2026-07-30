<!--
  Button — shadcn-svelte-style primitive (tailwind-variants + cn), styled to
  match the EasyQuran button system: primary / accent / ghost / quiet, plus the
  ink variants used on accent CTA panels. Renders <a> when `href` is set,
  otherwise <button>. Set `arrow` to append the self-translating "→".

  API is a superset of the stock shadcn-svelte Button so registry components
  (e.g. SidebarTrigger) that use `bind:ref` / spread rest props type-check
  against it: bindable `ref`, optional `children`, intersection (not union)
  element-attribute type.
-->
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
    /** append a "→" that nudges right on hover */
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
