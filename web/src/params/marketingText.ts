import { MARKETING_ROUTES } from "$lib/config/site-structure";

/** Marketing text-variant slugs (`.md`/`.txt` twins): `index` for the home
 * page plus every top-level marketing page. Keeps `[slug=marketingText].md`
 * and `[slug].txt` disjoint from the reader `[surah=surahSlug]` family. */
export function match(value: string): boolean {
  if (value === "") return false;
  const page = value === "index" ? "/" : `/${value}`;
  // SAFETY: widening the literal route values to string[] only loosens the
  // membership test; `page` is an untrusted string param, never a caller type.
  return (Object.values(MARKETING_ROUTES) as readonly string[]).includes(page);
}
