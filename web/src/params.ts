import { defineParams } from "@sveltejs/kit/params";

import { MARKETING_ROUTES } from "#lib/config/site-structure.js";

/** Marketing text-variant slugs (`.md`/`.txt` twins): `index` for the home
 * page plus every top-level marketing page. Keeps `[slug=marketingText].md`
 * and `[slug].txt` disjoint from the reader `[surah=surahSlug]` family. */
export function matchMarketingText(value: string): string | undefined {
  if (value === "") return undefined;
  const page = value === "index" ? "/" : `/${value}`;
  // SAFETY: widening the literal route values to string[] only loosens the
  // membership test; `page` is an untrusted string param, never a caller type.
  const known = (Object.values(MARKETING_ROUTES) as readonly string[]).includes(page);
  return known ? value : undefined;
}

/** Client-safe surah-slug matcher for `[surah=surahSlug].md`. SvelteKit runs
 * param matchers in the client router too, so this cannot import
 * `#lib/server/quran-data.js` (node:fs); it mirrors the exact grammar instead —
 * keep in lockstep with SURAH_SEGMENT + RESERVED_SURAH_SEGMENTS in
 * src/lib/i18n/reader.ts (baked slugs are letter-initial kebab words).
 *
 * The reject set is the union of every other top-level single-segment route
 * name (reader reserved words, product pages, marketing text slugs) so
 * `[slug=marketingText].md` and `[surah=surahSlug].md` stay disjoint by
 * construction at the same depth. Unknown letter-initial words still pass and
 * 404 through requireSurah — the same UX as an unknown slug today. */
const REJECT = new Set([
  // reader reserved segments (reader.ts RESERVED_SURAH_SEGMENTS)
  "juz",
  "page",
  "hizb",
  "rub",
  "t",
  "surah",
  "pages",
  "yours",
  // bounded product pages
  "search",
  "settings",
  "bookmarks",
  // marketing text-variant slugs (params.ts matchMarketingText)
  "index",
  "about",
  "faq",
  "contact",
  "privacy",
  "terms",
]);
const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

export function matchSurahSlug(value: string): string | undefined {
  if (!SURAH_SEGMENT.test(value) || REJECT.has(value)) return undefined;
  return value;
}

// Kit 3 collects every matcher in one `src/params.ts` module (the
// `src/params/` directory is gone). A matcher returns the parsed param value
// or `undefined` when it does not match; both matchers here are pure filters,
// so they echo the input back unchanged.
export const params = defineParams({
  marketingText: matchMarketingText,
  surahSlug: matchSurahSlug,
});
