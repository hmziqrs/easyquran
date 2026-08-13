import { resolve } from "$app/paths";
import type { Pathname, ResolvedPathname } from "$app/types";

// SAFETY: `resolve` is generic over a *single* literal route and rejects the
// `Pathname` union palette entries carry; the runtime only prefixes the
// configured base path, so widening is sound. Two flat casts because the
// generic signature is not directly comparable to the union signature.
const resolveAny = resolve as unknown;
// SAFETY: carried over from `resolveAny` above — same function value, only
// re-branded to the union signature the palette needs.
const resolveWide = resolveAny as (pathname: Pathname) => ResolvedPathname;

export const resolveHref = (href: Pathname): ResolvedPathname => resolveWide(href);
