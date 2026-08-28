<script lang="ts">
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  /**
   * Coloured-card primitive (plan 03 step 4): the boards' edge-to-edge metric strip card.
   * Takes a hue SLOT INDEX (1–4), never a colour prop — fill/fg resolve through the
   * --hue-N / --on-hue-N palette tokens (§61: no hard-coded colours, all 8 palette×mode
   * blocks work). The icon holder is the boards' 8px square (rounded-sm = --radius-sm).
   * State matrix (plan 03): hover = brightness(1.08) — the only state needing a filter,
   * because there is no second fill per hue; lightens in both modes (dark fills are deep,
   * brightness-up stays legible). Non-interactive: no focus/disabled states by design —
   * the metric strip is display, the landing CTA lives outside it.
   */
  type Hue = 1 | 2 | 3 | 4;

  /* Lookup tables (never ternary chains): hue index → token var strings. `satisfies`
     keeps the literal evidence while proving the 1–4 contract. */
  const FILL = {
    1: "var(--hue-1)",
    2: "var(--hue-2)",
    3: "var(--hue-3)",
    4: "var(--hue-4)",
  } as const satisfies Record<Hue, string>;
  const ON = {
    1: "var(--on-hue-1)",
    2: "var(--on-hue-2)",
    3: "var(--on-hue-3)",
    4: "var(--on-hue-4)",
  } as const satisfies Record<Hue, string>;

  type Props = {
    /** Which brand hue slot this card fills (1–4). */
    hue: Hue;
    /** Large numeral (or word) — 44px/800, tabular where numeric. */
    value: string;
    /** Card title (h3 ramp role). */
    label: string;
    /** Secondary line under the label (caption ramp role, 88% of the on-fill colour). */
    caption?: string;
    class?: string;
    /** Icon — rendered inside the 46px square holder, stroked in the on-fill colour. */
    children?: Snippet;
  } & HTMLAttributes<HTMLDivElement>;

  let { hue, value, label, caption, class: className, children, ...rest }: Props = $props();

  let fill = $derived(FILL[hue]);
  let on = $derived(ON[hue]);
  /* Boards: white at 20% over the fill (black/12 on light amber — on-hue-4 is near-black
     there, so mixing the ON token tracks the board in every block without a colour prop). */
  let holderBg = $derived(`color-mix(in oklab, ${on} 20%, transparent)`);
  let captionColor = $derived(`color-mix(in oklab, ${on} 88%, transparent)`);
</script>

<div
  class={cn(
    "flex flex-col justify-between gap-8 px-9 pt-10 pb-10 transition-[filter] duration-150 ease-out hover:brightness-[1.08]",
    className,
  )}
  style:background={fill}
  style:color={on}
  {...rest}
>
  <div class="flex items-center justify-between">
    <div
      class="flex size-[46px] items-center justify-center rounded-sm [&_svg]:size-[23px]"
      style:background={holderBg}
      style:color={on}
    >
      {@render children?.()}
    </div>
    <span class="text-[44px] leading-none tracking-[-0.05em] font-extrabold">{value}</span>
  </div>
  <div class="flex flex-col gap-[5px]">
    <span class="text-h3">{label}</span>
    {#if caption}<span class="text-caption" style:color={captionColor}>{caption}</span>{/if}
  </div>
</div>
