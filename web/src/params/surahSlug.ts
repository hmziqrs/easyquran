/** Client-safe surah-slug matcher for `[surah=surahSlug].md`. SvelteKit runs
 * param matchers in the client router too, so this cannot import
 * `$lib/server/quran-data` (node:fs); it mirrors the exact grammar instead —
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
  // marketing text-variant slugs (params/marketingText.ts)
  "index",
  "about",
  "faq",
  "contact",
  "privacy",
  "terms",
]);
const SURAH_SEGMENT = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

export function match(value: string): boolean {
  return SURAH_SEGMENT.test(value) && !REJECT.has(value);
}
