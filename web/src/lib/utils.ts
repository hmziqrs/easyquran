import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export function externalLinkAttrs(
  href?: string,
  opts: { me?: boolean } = {},
): { target?: "_blank"; rel?: string } {
  if (typeof href === "string" && /^https?:\/\//i.test(href)) {
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
