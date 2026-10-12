# Completed work — overviews

Short records of finished plans and audits. Leftovers that are still open are listed per
section, re-checked against the code on 2026-10-12.

## Design-system migration (landed 2026-08-31)

Moved the web app from "Sacred Editorial" (forest/ivory/gold, Newsreader + Inter) to the
pill/cobalt system. The current truth is [`design-system.md`](design-system.md) (v2).

| Phase | Shipped                                                                                        | Commits                |
| ----- | ---------------------------------------------------------------------------------------------- | ---------------------- |
| 0     | Visual harness (capture/contrast/states/matrix scripts, `/design/tokens`), oklch contrast gate | `aee24786`, `22d86b6f` |
| 1     | Zero-chroma neutrals × 8 palettes, mode-dependent hues, accent remap, zero elevation           | `868dbd5a`, `2a503239` |
| 2     | Nunito self-hosted, type ramp; Inter/Newsreader dropped                                        | `1c873fc4`             |
| 3     | Radius scale (pill / 8px holders / 10px blocks), primitives, state matrix                      | `79a2271c`             |
| 4     | `Band` full-bleed primitive, responsive ladder, RTL logical-property sweep + guard             | `80b5c183`…`ab35de0a`  |
| 5     | Marketing rebuilt to the 8-band board, hero pill search, hue metric strip                      | `4fcf0b9a`…`7c9204bc`  |
| 6     | App + reader chrome tokens (reading column untouched)                                          | `a5bfb525`…`5cb70e96`  |
| 7     | `design-system.md` rewritten to v2                                                             | `9d39360b`, `9f374f0e` |

The guards it added still run: contrast (`token-contrast.test.ts`), fonts (`fonts.test.ts`),
geometry (`geometry.test.ts`), RTL logical properties (`logical-properties.test.ts`).

**Still open** (owner judgement calls, none blocking):

- `Section.svelte` (`web/src/lib/components/layout/`) has no consumers beyond its barrel export
  and guard test — delete, then judge `Container`.
- Dead tokens `--pop` / `--pop-soft` / `--accent-line` (`layout.css`, `theme/derive.ts`) — delete
  with their aliases.
- `bg-accent`/`text-accent` utilities carry a dual meaning (`--color-accent` = primary vs
  `--accent` = hue-2, design-system §8) — retire.
- Ratify the warm sepia reader (option a) or add a separate warmth setting (option b).
- Band heights drift −4…−12px from the boards at 1440px; small contrast nits (faint dark juz chip,
  light hue-2 numerals 4.09:1); harness hardening (V8 comparator, `states.ts` race, `--out`).
- Resolved since: CI now runs the web/Rust suites (`.github/workflows/images.yml`); the "Powered by
  Gemini" watermark no longer exists in `web/src`.

## Primary translation removal (`502effc2`)

Removed the "primary translation" concept end to end — behaviour, code, copy, tests and persisted
state. Motivating bug: the primary-translation switch could **replace the Arabic text** in verse
mode on `/t/**` routes. The `readerSource` store and the `easyquran.reader.source` key were
deleted (one-shot cleanup on layout mount); modal rows became uniform toggle rows with a trailing
"Go to this translation" link; wording is "reading translation"/"main", never "primary".

**Still open:** translated **range** routes (`/t/{lang}/{translator}/{page|juz|hizb}/{n}`) render
verse rows from the route translation with no Arabic companion — `createArabicCompanion` is used
only by `SurahReader`, not `RangeReader`. Left out of scope as a separate render-mode decision.

## Navigation audit and URL scheme A (implemented)

Two plans that set today's reader addressing. The current contract lives in
[`quran-system.md`](quran-system.md) (Part 5 divergence #10) and the guarded
`surah*For(ctx, …)` helper family (`web/src/lib/data/quran.ts`).

- **One global page numbering** (Madani pages 1–604) everywhere: labels, pills, sidebar, sticky
  position indicator, URLs, titles. Surah-local page URLs were removed — one URL per surah, scroll
  position mirrored in `?v={surah}:{ayah}`, never the path.
- **No route prefixes**: `en` is unprefixed, `ar` is `/ar/`-prefixed, the `/app` marker and the
  `/app` hub are gone (`/app` → 308 `/surah`; resume lives on `/yours`). Families: `/{surah}`,
  `/{surah}/t/{lang}/{translator}`, `/page|juz|hizb/{n}`, `/t/{lang}/{translator}/page|juz|hizb/{n}`,
  `/surah`, `/juz`, `/pages`, plus `/search`, `/settings`, `/bookmarks`, `/yours`.
- **Redirects**: numeric alias `/2` → 308 `/al-baqarah`; legacy `/app/*` and `/en/*` → 308;
  rubʿ routes (`/rub/N`, `/t/…/rub/N`) were later retired to a 308 to the quarter's first ayah
  (`5d54ca6b`).
- **SEO hygiene**: sitemap lists reader `<loc>`s only (no hreflang alternate flood, no noindex
  hub); translated routes are SSR + 7-day disk cache, never prerendered.
- Sidebar Ayah tab works on every route; index pages keep the reader's translation context.

## Design enhancement study (2026-10-08, not adopted)

A standalone HTML gallery (`9d9c2ae8`) compared "color cards"
(solid accent cards, 18–20px corners) and "sharper outlines" (white cards, colored edges, 12px)
for the homepage and reader, with Nunito vs Manrope. **Not adopted:** its weights above 600 and
12–20px radii violate the design-system guards (§10/§13). Homepage/card work continues under v2
constraints in [`next-features.md`](next-features.md) Feature 5.
