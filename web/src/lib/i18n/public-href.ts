import { base } from "$app/paths";

export type PublicHref = `/${string}`;

/** Resolve a validated public path that SvelteKit's generated route types cannot see after rerouting. */
export function publicHref(href: PublicHref): string {
  if (href.startsWith("//") || href.includes("\\") || /\p{Cc}/u.test(href)) {
    throw new TypeError(`Invalid public href: ${href}`);
  }
  return `${base}${href}`;
}
