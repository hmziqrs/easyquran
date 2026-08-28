# 03 — Geometry and primitives

## Goal

Move to the boards' geometry — pill controls, 8px square icon holders, 10px blocks —
across `ui/` and the shared components, and design the interaction states the mockups
do not draw.

## Current state

Radius scale (`layout.css:70–75`):

```css
--radius-sm: 8px;  --radius-md: 12px;  --radius-lg: 18px;
--radius-xl: 24px; --radius-pill: 999px;
```

Usage across `web/src` (`.svelte` files):

| Class | Count |
| --- | --- |
| `rounded-md` | 65 |
| `rounded-full` | 40 |
| `rounded-lg` | 35 |
| `rounded-xl` | 20 |
| `rounded-pill` | 15 |
| one-off `rounded-[Npx]` | ~25 across 9 distinct values |

Primitives live in `web/src/lib/components/ui/` — `button`, `icon-button`, `input`,
`tabs`, `textarea`, `label`, plus `separator`, `sheet`, `sidebar`, `skeleton`, `tooltip`,
`accordion`, `command`. `Chip` sits outside at `components/chip/Chip.svelte`.

`button-variants.ts` is a `tailwind-variants` table: seven variants
(`primary`, `secondary`, `ghost`, plus legacy `accent`, `quiet`, `ink`, `outline-ink`)
and five sizes. It is guarded by `ui/__tests__/primitives.test.ts`, which asserts exact
class strings — for example `bg-primary text-primary-foreground` and
`hover:bg-primary-hover`.

## Target state

### The geometry rule, stated once

| Element class | Radius | Rationale |
| --- | --- | --- |
| **Controls** — button, search field, chip, tab, ⌘K badge | `999px` | The boards' defining move |
| **Icon holders** — the square behind an icon | `8px` | Deliberately *not* circular; this is what stops it reading as generic |
| **Blocks** — rows, cards, panels, popovers, sheets | `10px` | Tight, not bubbly |
| **Numerals** — surah number chips | `10px` rounded rect, `min-width` | Three digits must not crowd a circle |
| **Headline highlight** | `16px` | A pill at 76px becomes a lozenge and swallows the words |

### Token changes

Redefining the scale is far cheaper than editing 160 call sites, and most current usage
maps cleanly:

```css
--radius-sm:   8px;    /* unchanged — now means "icon holder" */
--radius-md:   10px;   /* was 12 — now means "block" */
--radius-lg:   10px;   /* was 18 — collapses onto block */
--radius-xl:   14px;   /* was 24 — large panels only */
--radius-pill: 999px;  /* unchanged */
--radius-highlight: 16px; /* new */
```

`rounded-md` (65 uses) and `rounded-lg` (35 uses) both landing on 10px does most of the
work for free. What still needs hand editing:

- every **control** currently on `rounded-md` → `rounded-pill`;
- every **icon holder** currently on `rounded-full` → `rounded-sm`;
- the ~25 one-off `rounded-[Npx]` values → the nearest token, or justified in a comment.

`rounded-full` at 40 uses is the dangerous one: some are icon holders (must become
squares), some are avatars and status dots (must stay round). It cannot be done by
find-and-replace — audit each.

### The state matrix

The mockups draw one state. These must be designed once, here, and applied uniformly.

| | Rest | Hover | Active | Focus-visible | Disabled |
| --- | --- | --- | --- | --- | --- |
| **Primary button** | `--primary` / `--primary-foreground` | `--primary-hover` | `translate-y-px` | 2px `--focus-ring`, 2px offset | `opacity-50`, no pointer events |
| **Secondary** | `--surface` + `--border` | `--surface-hover` + `--border-strong` | as above | as above | as above |
| **Ghost** | transparent | `--surface-hover` | as above | as above | as above |
| **Chip** | `--surface` + `--border` | `--surface-hover` | — | ring | — |
| **Row (surah)** | `--surface` + `--border` | `--surface-hover` + `--border-strong` | — | ring | — |
| **Coloured card** | `--hue-N` / `--on-hue-N` | `brightness(1.08)` | — | ring in `--on-hue-N` | — |

The focus ring keeps its current form (`focus-visible:outline-2 outline-offset-2
outline-focus-ring`, `button-variants.ts:base`) — it already works and is the one state
with an accessibility obligation.

Hover on a coloured card is the only case needing a filter rather than a token, because
there is no second fill per hue. Keep it to `brightness`, and check it in dark mode where
the fills are already deep.

## Steps

1. **Retune the radius tokens** (above). Land alone; the whole app shifts slightly and
   nothing else changes.
2. **Audit `rounded-full`.** Produce the list of 40 and split into icon-holder vs
   genuinely-round. Record the split in the commit message.
3. **Primitives, one file per commit**, in this order — least to most consumed:
   `chip` → `input`/`textarea` → `tabs` → `icon-button` → `button`.
   Each keeps its variant names and its API; only classes change.
4. **Add the coloured-card primitive.** The boards' metric card has no equivalent in
   `ui/`. Add `components/card/MetricCard.svelte` (or extend `Card`) taking a
   `hue: 1 | 2 | 3 | 4`, and resolving `--hue-N` / `--on-hue-N` — never a colour prop, so
   the §61 guard stays satisfiable.
5. **Apply the state matrix** across the primitives in one pass, after the classes settle.

## Verification

### Machine

- `primitives.test.ts` — update the asserted class strings, and **extend** with:
  - controls carry `rounded-pill`, never `rounded-md`;
  - icon holders carry `rounded-sm`, never `rounded-full`;
  - every variant defines hover **and** focus-visible;
  - the existing §61 no-hex / no-Tailwind-palette assertions stay (the new colours are
    tokens, so this is free).
- `chip.test.ts` — same treatment.
- New guard `geometry.test.ts`: no `.svelte` file under `lib/components/ui/` contains a
  `rounded-[Npx]` literal. Forces every radius through the scale.
- `pnpm lint` will catch a nested ternary if a variant table is refactored into one —
  the radius/hue choice must be a lookup table, not `?:` chains.

### Visual

Plan 07 check **V5** (primitive board): a page rendering every primitive × every variant ×
every state × light/dark, screenshotted and diffed between commits. Because states cannot
be captured by a static screenshot, the harness drives hover and focus explicitly — see
plan 07.

## Risks

- **`rounded-full` mis-audited** is the most likely visible bug: a circular avatar turned
  into a square, or an icon holder left round. The split list in step 2 is the artefact
  that prevents it.
- **`primitives.test.ts` asserts exact strings.** Every class change breaks it by design.
  Update the guard in the *same* commit as the primitive, never separately, or the failure
  gets normalised.
- **Radius retuning is global.** `--radius-lg` moving 18 → 10 is an 8px change on 35 call
  sites, some of which are large panels where 10px may look mean. If a panel needs more,
  it uses `--radius-xl` — it does not get a one-off.
- **Legacy button variants** (`ink`, `outline-ink`, `quiet`, `accent`) exist for
  compatibility. Do not delete them here; that is a separate cleanup with its own grep.
