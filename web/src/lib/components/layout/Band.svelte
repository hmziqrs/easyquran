<script lang="ts">
  import { cn } from "$lib/utils";
  import type { HTMLAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";

  /**
   * Full-bleed band (plan 04): owns a background edge to edge; its content stops at
   * the §15 ramp gutter (`--gutter`) inside a capped column. Bands never gain a
   * horizontal margin — a band with side margins is a card again. The band owns its
   * container: never wrap one in a `Container` and never pass one in (full-bleed plus
   * a max-width column double-nests). Separation between bands is the tone changing;
   * `rule` is the hairline escape hatch for where it does not.
   */
  type Tone = "page" | "panel" | "accent" | "surface";
  type Pad = "default" | "tight" | "none";
  type Width = "default" | "narrow" | "wide" | "full";

  type Props = {
    /** Background the band paints. "page" paints nothing — see TONE below. */
    tone?: Tone;
    /** Vertical rhythm: 96 (default) / 64 (tight) at desktop, ramping down (§15). */
    pad?: Pad;
    /** Inner column cap; "full" = gutter only, no cap. */
    width?: Width;
    /** Hairline top border, for where the tone does not change. */
    rule?: boolean;
    /** Classes for the inner column (override the ramp gutter/cap through tailwind-merge). */
    contentClass?: string;
    children: Snippet;
  } & HTMLAttributes<HTMLElement>;

  let {
    tone = "page",
    pad = "default",
    width = "default",
    rule = false,
    class: className = "",
    contentClass = "",
    children,
    ...rest
  }: Props = $props();

  /* Lookup tables (never ternary chains). "page" is transparent on purpose: the page
     ground already is --background (html/body), and painting it again would both fight
     any non-page parent and break Section's byte-stable passthrough. "accent" also sets
     its on-fill text colour — the token pair is inseparable. */
  const TONE = {
    page: "",
    panel: "bg-background-subtle",
    accent: "bg-primary text-primary-foreground",
    surface: "bg-surface",
  } as const satisfies Record<Tone, string>;

  const PAD = {
    default: "py-(--band-pad)",
    tight: "py-(--band-pad-tight)",
    none: "",
  } as const satisfies Record<Pad, string>;

  const CAP = {
    default: "max-w-[1200px]",
    narrow: "max-w-[880px]",
    wide: "max-w-[1440px]",
    full: "",
  } as const satisfies Record<Width, string>;

  let toneClass = $derived(TONE[tone]);
  let padClass = $derived(PAD[pad]);
  let capClass = $derived(CAP[width]);
</script>

<section class={cn(toneClass, padClass, rule && "border-t border-border", className)} {...rest}>
  <div class={cn("mx-auto w-full px-(--gutter)", capClass, contentClass)}>{@render children()}</div>
</section>
