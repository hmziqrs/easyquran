import { resolve } from "$app/paths";
import type { Pathname, ResolvedPathname } from "$app/types";

/**
 * `resolve` is generic over a *single* literal route, so it rejects the
 * `Pathname` union that palette entries carry — sources span many routes. The
 * runtime only ever prefixes the configured base path, so widening the
 * parameter here is sound; this is the one cast that states it.
 */
export const resolveHref = (href: Pathname): ResolvedPathname =>
  (resolve as unknown as (pathname: Pathname) => ResolvedPathname)(href);
