import { tv, type VariantProps } from "tailwind-variants";

// Kept in a plain .ts file (not the component's module script) so the barrel
// can re-export it without tripping TS2614 on `*.svelte` modules.
export const buttonVariants = tv(
  {
    base: "group inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md border border-transparent font-sans tracking-tight transition-[background-color,border-color,color,transform,filter] duration-150 ease-out active:translate-y-px focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:pointer-events-none disabled:opacity-50",
    variants: {
      variant: {
        primary: "bg-fg text-bg border-fg hover:bg-fg-2 hover:border-fg-2",
        accent: "bg-accent text-accent-fg border-accent hover:brightness-110",
        ghost: "bg-transparent text-fg border-line-2 hover:bg-bg-2 hover:border-line-3",
        quiet: "bg-transparent text-fg-2 px-2 hover:text-fg",
        ink: "bg-accent-fg text-accent border-accent-fg hover:brightness-105",
        "outline-ink": "bg-transparent text-accent-fg border-accent-fg hover:bg-black/5",
      },
      size: {
        sm: "h-[30px] px-2.5 text-xs",
        md: "h-9 px-3.5 text-sm",
        lg: "h-11 px-[18px] text-base",
        icon: "size-9 p-0",
        "icon-sm": "size-7 p-0",
      },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
  { twMerge: true },
);

export type ButtonVariant = VariantProps<typeof buttonVariants>["variant"];
export type ButtonSize = VariantProps<typeof buttonVariants>["size"];
