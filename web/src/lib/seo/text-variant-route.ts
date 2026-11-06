/**
 * Marketing text variants (`/<slug>.md`, `/<slug>.txt`). Shape only: the route
 * matchers in src/params.ts decide which slugs are real marketing pages.
 */
const TEXT_VARIANT_PATH = /^\/[a-z0-9][a-z0-9-]*\.(?:md|txt)$/u;

/**
 * True for `/ar/<slug>.md|.txt` whose unprefixed candidate is a text-variant path.
 * The reroute maps it onto the unprefixed route; the locale stays on the request
 * URL, which the route reads to choose the Arabic source page.
 */
export function isLocalizedTextVariant(rawPathname: string, candidatePathname: string): boolean {
  if (!rawPathname.startsWith("/ar/")) return false;
  return rawPathname.slice(3) === candidatePathname && TEXT_VARIANT_PATH.test(candidatePathname);
}
