import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

// shadcn-svelte `cn` helper — merge conditional classes and de-dupe Tailwind conflicts.
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/**
 * Anchor attributes that route an external link to a new tab. Absolute http(s)
 * URLs get `target="_blank"` + `rel="noopener noreferrer"`; relative paths,
 * hash links, and `mailto:`/`tel:` schemes stay in-tab. Returns `{}` when
 * there's nothing to add, so it spreads cleanly onto any `<a>`.
 */
export function externalLinkAttrs(href?: string): { target?: "_blank"; rel?: string } {
  if (typeof href === "string" && /^https?:\/\//i.test(href)) {
    return { target: "_blank", rel: "noopener noreferrer" };
  }
  return {};
}

export type WithElementRef<T, U = HTMLElement> = T & { elementref?: U | null };
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, "children"> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
