import { resolve } from "$app/paths";

export type PublicHref = `/${string}`;

// SAFETY: `resolve` is generic over a single literal route/path and rejects the
// wide `PublicHref` shapes this helper is built for (rerouted reader and
// marketing paths the generated `Path` union cannot see). The runtime pathname
// branch only prefixes the base path, so widening is sound; two flat casts
// because the generic signature is not directly comparable to this one.
const resolveAny = resolve as unknown;
// SAFETY: carried over from `resolveAny` above — same function value, only
// re-branded to the public-href signature.
const resolvePublicHref = resolveAny as (pathname: string) => string;

/** Resolve a validated public path that SvelteKit's generated route types cannot see after rerouting. */
export function publicHref(href: PublicHref): string {
  if (href.startsWith("//") || href.includes("\\") || /\p{Cc}/u.test(href)) {
    throw new TypeError(`Invalid public href: ${href}`);
  }
  // kit 3 removed `base` from `$app/paths`. The pathname branch (argument
  // without a leading slash) prepends the base path and never consults the
  // route table — exactly the old `${base}${href}` behaviour.
  return resolvePublicHref(href.slice(1));
}
