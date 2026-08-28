import { tv, type VariantProps } from "tailwind-variants";

/**
 * §35 (docs/design-system.md): primary/secondary/ghost roles, 44px min target on the
 * default size, 10–12px radius (rounded-md = --radius-md 12px), primary text pairs with
 * `--primary-foreground` — never an assumed white. Semantic tokens only (§61): every
 * class below resolves through the palette blocks, so all 4 palettes × light/dark work.
 * Legacy variant names (accent/quiet/ink/outline-ink) are kept as aliases so existing
 * consumers keep rendering; their styles now map onto the §35 roles.
 */
export const buttonVariants = tv(
  {
    base: "group inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md border border-transparent font-sans tracking-tight transition-[background-color,border-color,color,transform,filter] duration-150 ease-out active:translate-y-px focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring disabled:pointer-events-none disabled:opacity-50",
    variants: {
      variant: {
        primary:
          "bg-primary text-primary-foreground font-[550] hover:bg-primary-hover",
        secondary:
          "bg-surface text-foreground border-border hover:bg-surface-hover hover:border-border-strong",
        ghost: "bg-transparent text-foreground hover:bg-surface-hover",
        /* Legacy interactive-accent CTA — now the §35 primary role. */
        accent:
          "bg-primary text-primary-foreground font-[550] hover:bg-primary-hover",
        quiet: "bg-transparent text-foreground-secondary px-2 hover:text-foreground",
        ink: "bg-primary-foreground text-primary hover:brightness-105",
        "outline-ink":
          "bg-transparent text-foreground border-border-strong hover:bg-surface-hover",
      },
      size: {
        /* sm stays compact for dense table/inline contexts (§51 44px applies to core CTAs). */
        sm: "h-9 px-3 text-caption",
        md: "min-h-11 px-[18px] text-body-s",
        lg: "min-h-12 px-6 text-body",
        icon: "size-11 p-0",
        "icon-sm": "size-9 p-0",
      },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
  { twMerge: true },
);

export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
export type ButtonSize = VariantProps<typeof buttonVariants>["size"];
