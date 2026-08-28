import { tv, type VariantProps } from "tailwind-variants";

/**
 * §36/§51 (docs/design-system.md): one line icon family (Lucide), icon sizes 18/20/24,
 * stroke ~1.75, 44×44 target on the default size. Icon geometry is forced onto the
 * slotted SVG via CSS ([&_svg]) so consumers cannot drift from the family sizing, and
 * CSS `stroke-width` overrides Lucide's presentation attribute. Semantic tokens only (§61).
 */
export const iconButtonVariants = tv(
  {
    base: "inline-flex shrink-0 cursor-pointer items-center justify-center border border-transparent bg-transparent font-sans transition-[background-color,border-color,color,transform] duration-150 ease-out active:translate-y-px focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:shrink-0",
    variants: {
      variant: {
        ghost: "text-foreground-secondary hover:bg-surface-hover hover:text-foreground",
        secondary:
          "bg-surface text-foreground border-border hover:bg-surface-hover hover:border-border-strong",
        primary: "bg-primary text-primary-foreground hover:bg-primary-hover",
      },
      size: {
        /* Plan 03 geometry: controls are pills — 999px on a square target renders the
           board's circular icon button (44×44 moon toggle). */
        sm: "size-9 rounded-pill [&_svg]:size-[18px] [&_svg]:stroke-[1.75]",
        md: "size-11 rounded-pill [&_svg]:size-5 [&_svg]:stroke-[1.75]",
        lg: "size-12 rounded-pill [&_svg]:size-6 [&_svg]:stroke-[1.75]",
      },
    },
    defaultVariants: { variant: "ghost", size: "md" },
  },
  { twMerge: true },
);

export type IconButtonVariant = VariantProps<typeof iconButtonVariants>["variant"];
export type IconButtonSize = VariantProps<typeof iconButtonVariants>["size"];
