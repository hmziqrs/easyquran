# 05 — Marketing and the landing page

## Goal

Rebuild `/` to match the boards, and add the i18n keys the new sections need without
breaking locale parity.

## Current state

`web/src/routes/(marketing)/+page.svelte` (147 lines), five sections:

1. Hero — badge pill, h1, intro, two CTA buttons (`Button variant="accent"` + `ghost`)
2. "Today" — eyebrow, h2, intro, 4 value cards from `landing.values`
3. Surah list — all 114 from `data.surahs`, 3-column grid, rotated-45° number badge
4. Roadmap — eyebrow, h2, 4 cards from `landing.roadmap`
5. (Footer via `(marketing)/+layout.svelte`)

Copy resolves through `lib/i18n/landing-copy.ts` → `$lib/i18n/m/landing`, backed by
`web/messages/{en,ar}.json`. `landing.values[].chip` currently carries **Tailwind classes**
(`"bg-accent text-accent-fg"`, `landing-copy.ts:56`) — presentation living in the copy
layer, which the new hue tokens make untenable.

## Target state — section by section

Per the boards, in order:

| # | Band | `tone` | Notes |
| --- | --- | --- | --- |
| 1 | Header | `surface` | Wordmark, nav, inline pill search with ⌘K, mode toggle, CTA. Bottom hairline. |
| 2 | Hero | `page` | h1 76px with a 16px accent highlight on "simplest way"; lead; **pill search field** with in-field Search button; "Often opened" chips |
| 3 | Metric strip | — | **Gapless, full width, four hues.** Surahs 114 / Juz 30 / Pages 604 / Bookmarks |
| 4 | Index | `page` | Eyebrow, h2, lead, "See all 114"; 3-column surah rows |
| 5 | Why | `panel` | Two columns: copy + CTA left; three rule-separated steps right |
| 6 | Roadmap | `page` | Four columns divided by 1px rules, **not** cards |
| 7 | Closing | `accent` | Bismillah, h2, lead, inverted CTA, reassurance line |
| 8 | Footer | `surface` | Unchanged structure, new tokens |

### The hero search is the page's primary action

It replaces the CTA pair. It must be a **real control**, not a decorative div: a labelled
input that submits to the existing search route, keyboard reachable, with the ⌘K hint
bound to the same palette the app already opens (`lib/components/search/`,
`lib/hotkeys.svelte.ts`).

Per `AGENTS.MD`: keyboard shortcuts go through **TanStack Hotkeys** via
`registerHotkey()`, dynamically imported from the consumer — never a hand-rolled
`keydown` listener, and never statically imported into an always-mounted component.

### The bookmarks card shows real bookmarks

The fourth metric card has no number — it lists two example bookmarks. In the product it
should show the reader's **actual** most recent bookmarks (`lib/bookmarks/store.ts`), with
an empty state for a new visitor. Decide the empty state before building: a prompt
("Bookmark an ayah and it appears here") is better than fabricated samples.

### Copy and i18n

- **Remove presentation from copy.** `landing.values[].chip` stops carrying Tailwind
  classes; the value gains a `hue: 1 | 2 | 3 | 4` and the component resolves the token.
- **New keys needed** — metric strip labels and sub-lines, the search placeholder, "Often
  opened", the eyebrows, the closing band. Draft in `en.json`; **`ar.json` must gain every
  key in the same commit** or `pnpm i18n:check` (which runs in `prebuild`, `prelint` and
  `pretest`) fails.
- Existing copy is already right for the design and should not be rewritten. The hero
  headline, the four values, the roadmap entries and the footer blurb carry over verbatim.
- Placeholder that must not ship: the boards' footer credit
  `Built by [YOUR NAME] · [@YOURHANDLE]`. The real values come from `getOwnerPublic()`
  (`lib/server/owner.ts`) via `OwnerPublic`.

### Surah rows

Number chips become rounded rects with `min-width` (plan 03) — the current 45°-rotated
badge goes. Ayah counts and names keep coming from the catalogue, never hard-coded.

**Hrefs do not change.** They must keep using `surahPathFor(ctx, s)` with `ctx` from the
route context, per `AGENTS.MD` and `nav-guard.test.ts`. Restyling a row must not touch its
`href` construction.

## Steps

1. **Port the footer and header first.** They appear on every marketing page, so they
   validate the tokens and geometry across five pages at once.
2. **Hero band**, without the search field — static first, so layout and type land alone.
3. **Wire the search field** to the existing search route and hotkey.
4. **Metric strip.** New `MetricCard` (plan 03), gapless grid, real bookmark data.
5. **Index band** — rows restyled, hrefs untouched.
6. **Why band** — two columns, rule-separated steps.
7. **Roadmap band** — rules, not cards.
8. **Closing band** — accent tone, inverted CTA.
9. **Re-check the other four marketing pages** (`about`, `faq`, `contact`, `privacy`,
   `terms`) — they inherit the tokens automatically but their layouts are untouched, so
   they need a visual pass for anything that assumed the old ground.

## Verification

### Machine

- `pnpm i18n:check` — locale parity for every new key.
- `marketing-surface-guard.test.ts` — extend to cover the new sections' copy resolution.
- `nav-guard.test.ts` — must stay green with no edits; if it fails, an href was
  hand-built.
- New guard: `landing.values[].hue` is a number, and no copy value in
  `landing-copy.ts` matches a Tailwind class pattern (`/\b(bg|text)-[a-z-]+\b/`). Prevents
  presentation leaking back into the copy layer.

### Visual

Plan 07 checks **V8** (landing diff) and **V9** (marketing sweep):

- **V8** — screenshot `/` at 1440 and compare against `design/PillLightCobalt.dc.html`
  rendered in the same browser at the same width, section by section, in both modes and
  all four palettes. This is the check the whole migration is aimed at.
- **V9** — the other five marketing pages at three widths, both modes, looking for
  contrast failures and orphaned styling from the old ground.

## Risks

- **The search field is a functional regression risk**, not a visual one. It replaces the
  primary CTA; if it does not submit correctly, the page loses its main action. Test the
  submit path and the hotkey before the visual polish.
- **Fabricated bookmarks would ship as fake product data.** Resolve the empty state
  explicitly.
- **i18n parity failures block the build**, including `pnpm lint` and `pnpm test` via their
  `pre*` hooks — so a missing Arabic key blocks everything, not just the build.
- **`data.surahs` renders all 114 rows.** The boards show 12. Confirm whether the page
  keeps all 114 (current behaviour, good for SEO and for `llms.txt`) or truncates with
  "See all 114" — the boards imply truncation, but truncating removes 100+ internal links
  from the highest-authority page. **Recommend keeping all 114** and treating the boards'
  12 as an abbreviation for presentation.
