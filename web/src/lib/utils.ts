import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * Type-ramp roles (plan 02: `--text-*` @theme keys in layout.css) are genuine font-size
 * utilities, but tailwind-merge's default classifier files any unknown `text-<word>`
 * under the text-COLOUR group — so a size role merged after a variant colour DELETED
 * `text-primary-foreground`/`text-primary` (vision r1 C3/C6 FAIL: invisible ink CTA,
 * low-contrast primary CTAs). Registering the roles in the font-size group makes them
 * conflict with other font-size utilities only, never with colour classes.
 *
 * tailwind-variants merges variant+size with its OWN default twMerge, so button-variants
 * passes this same config object to tv({ twMergeConfig }) — single source, defined here.
 */
export const RAMP_TEXT_ROLES = [
  "display-xl",
  "display-l",
  "h1",
  "h2",
  "h3",
  "body-xl",
  "body-l",
  "body",
  "body-s",
  "caption",
  "micro",
] as const;

export const rampTwMergeConfig = {
  extend: {
    classGroups: {
      "font-size": [{ text: [...RAMP_TEXT_ROLES] }],
    },
  },
} as const;

const twMergeWithRamp = extendTailwindMerge(rampTwMergeConfig);

export function cn(...inputs: ClassValue[]): string {
  return twMergeWithRamp(clsx(inputs));
}

export function formatBytes(n: number | null): string {
  if (n == null || n <= 0) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  if (n < 1024 ** 4) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  return `${(n / 1024 ** 4).toFixed(1)} TB`;
}

export type ExternalLinkAttrs = { target?: "_blank"; rel?: string };

export function externalLinkAttrs(
  href?: string,
  opts: { me?: boolean } = {},
): ExternalLinkAttrs {
  if (href !== undefined && /^https?:\/\//i.test(href)) {
    return {
      target: "_blank",
      rel: opts.me ? "me noopener noreferrer" : "noopener noreferrer",
    };
  }
  return {};
}

export type WithElementRef<T, U = HTMLElement> = T & { ref?: U | null; elementref?: U | null };
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
