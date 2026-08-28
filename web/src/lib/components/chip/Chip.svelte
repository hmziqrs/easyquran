<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = {
    /** Leading marker dot (editorial gold, §43). */
    dot?: boolean;
    /** Editorial gold badge tone (§19 Makki: accent-soft / accent-strong). */
    accent?: boolean;
    /** Transparent badge (no surface fill). */
    ghost?: boolean;
    /** Active state; with `onclick` this is exposed as aria-pressed, never color alone (§51). */
    active?: boolean;
    /** Provide to render an interactive §33 filter chip (button) instead of a static badge. */
    onclick?: HTMLButtonAttributes["onclick"];
    class?: string;
    children: Snippet;
  };

  let {
    dot = false,
    accent = false,
    ghost = false,
    active = false,
    onclick,
    class: className = "",
    children,
  }: Props = $props();

  /* §19 badge sizing: 24px height, 8px inline padding, 11/16 medium, pill radius. */
  const sizeClass =
    "inline-flex h-6 items-center gap-1.5 rounded-pill border px-2 text-[11px] font-medium leading-4";

  /* First-match tone (§19 badges / §33 inactive chips); named fn instead of nested ternaries. */
  function tone(): string {
    if (active) return "border-transparent bg-primary text-primary-foreground";
    if (accent) return "border-transparent bg-gold-soft text-gold-strong";
    if (ghost) return "border-transparent bg-transparent text-foreground-secondary";
    return "border-border bg-surface text-foreground-secondary";
  }

  /* Interactive §33 chips keep the hover affordance on the INACTIVE state only, so the
     active primary fill is not washed out by a hover surface. */
  function interactive(): string {
    const focusClass =
      "cursor-pointer transition-colors ease-out focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
    if (active) return focusClass;
    return `${focusClass} hover:bg-surface-hover hover:border-border-strong`;
  }
</script>

{#if onclick}
  <button
    type="button"
    aria-pressed={active}
    class={cn(sizeClass, tone(), interactive(), className)}
    {onclick}
  >
    {#if dot}<span class="size-1.5 rounded-full bg-gold"></span>{/if}
    {@render children()}
  </button>
{:else}
  <span class={cn(sizeClass, tone(), className)}>
    {#if dot}<span class="size-1.5 rounded-full bg-gold"></span>{/if}
    {@render children()}
  </span>
{/if}
