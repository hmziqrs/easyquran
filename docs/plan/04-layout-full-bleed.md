# 04 — Layout: full-bleed bands

## Goal

Replace the contained-section model with the boards' band model, and design the
responsive and RTL behaviour the fixed-width mockups do not express.

## Current state

`web/src/lib/components/layout/Section.svelte`:

```svelte
<section class={cn(tight ? "py-12" : "py-16 md:py-24", border && "border-t border-border", className)}>
  <Container {width}>{@render children()}</Container>
</section>
```

`Container.svelte` centres a max-width column with a fixed `px-6` gutter:

```
default: max-w-[1200px] · narrow: max-w-[880px] · wide: max-w-[1440px]
```

So today: the section is transparent, the column is capped, the gutter is 24px, and
sections are separated by a top border.

## Target state

A **band** owns a full-width background; its content stops at a gutter. Sections are
separated by their background changing, not by a rule.

```
┌──────────────────────────────────────────────────┐  ← background runs edge to edge
│        ┌──────────────────────────────┐          │
│  72px  │        content column        │   72px   │
│        └──────────────────────────────┘          │
└──────────────────────────────────────────────────┘
   96px vertical padding, top and bottom
```

From the boards: gutter **72px**, band padding **96px**, grid gutters **12–18px**.

### The `Band` component

New: `web/src/lib/components/layout/Band.svelte`.

| Prop | Values | Meaning |
| --- | --- | --- |
| `tone` | `"page"` \| `"panel"` \| `"accent"` \| `"surface"` | Which background the band paints |
| `pad` | `"default"` (96) \| `"tight"` (64) \| `"none"` | Vertical rhythm |
| `width` | `"default"` \| `"narrow"` \| `"wide"` \| `"full"` | Inner column cap; `full` = gutter only |
| `rule` | `boolean` | Optional hairline, for where tone does not change |

`Section` stays, reimplemented over `Band` with `tone="page"` and its current padding, so
no existing call site breaks. New surfaces use `Band` directly. Deprecate `Section` in a
comment rather than removing it.

### Responsive — the part the mockups do not answer

The boards are fixed at 1440px. Decide the ramp here rather than per-page:

| Viewport | Gutter | Band padding | Metric strip | Surah grid |
| --- | --- | --- | --- | --- |
| ≥ 1280 | 72 | 96 | 4 columns, gapless | 3 columns |
| 1024–1279 | 48 | 80 | 4 columns, gapless | 2 columns |
| 768–1023 | 32 | 64 | 2×2, gapless | 2 columns |
| < 768 | 20 | 48 | 1 column, stacked | 1 column |

Two rules that follow from the design rather than from convention:

- **The gapless metric strip stays gapless at every width.** Its whole character is that
  the four colours touch. Wrapping to 2×2 preserves that; introducing gutters does not.
- **Bands never gain a horizontal margin.** A band with side margins is a card again, and
  the card model is what this replaces.

### RTL

The mockups use `margin-left: auto` throughout — that is a mockup artefact, not a
specification. Every ported style uses logical properties:

| Never | Always |
| --- | --- |
| `margin-left` / `margin-right` | `margin-inline-start` / `margin-inline-end` |
| `padding-left` / `padding-right` | `padding-inline-*` |
| `text-align: left` | `text-align: start` |
| `border-left` | `border-inline-start` |
| `left` / `right` | `inset-inline-start` / `inset-inline-end` |

In Tailwind that means `ms-*`/`me-*`/`ps-*`/`pe-*`/`start-*`/`end-*`, never `ml-*`/`mr-*`.
Commit `54325a6d` already fixed a logical-inset bug in the app skip-link — the same class
of bug will reappear on every band ported carelessly.

Directional glyphs (the arrow in every CTA) must flip under RTL. The landing page already
does this for one icon (`+page.svelte:75`, `rotate-180` when `locale === "ar"`); make it a
property of the icon component rather than repeating the conditional.

## Steps

1. **Add `Band.svelte`** with the props above and no consumers. Land alone.
2. **Reimplement `Section` over `Band`**, preserving its rendered output byte for byte.
   Verify with the existing marketing pages — nothing should move.
3. **Add the responsive ramp** as tokens or a documented utility set, so pages do not each
   invent breakpoints.
4. **Port the landing page** to `Band` — plan 05.
5. **RTL sweep**: grep `\bm[lr]-`, `\bp[lr]-`, `\btext-(left|right)\b`, `\b(left|right)-\d`
   across `web/src`, and convert. This is mechanical but must be done in one pass so the
   grep stays clean afterwards.
6. **Add the RTL guard** (below).

## Verification

### Machine

- New guard `logical-properties.test.ts`: no `.svelte` file under
  `routes/(marketing)` or `lib/components/layout` uses a physical direction class.
  Start scoped to the migrated surfaces so it can land green, then widen.
- `pnpm check` — `svelte-check --fail-on-warnings` will catch a11y regressions introduced
  while restructuring sections (heading order, landmark nesting).

### Visual

Plan 07 checks **V6** (breakpoint sweep) and **V7** (RTL mirror):

- **V6** screenshots every migrated page at 1440 / 1280 / 1024 / 768 / 390 and asserts no
  horizontal scrollbar, no clipped band, and the metric strip's column count per the table.
- **V7** renders each page with `dir="rtl"` and the Arabic locale, and diffs against the
  LTR capture mirrored — differences that are *not* mirror-symmetric are the bugs.

## Risks

- **`Section` is consumed by every marketing page.** Reimplementing it over `Band` must be
  output-identical or the whole surface shifts. Verify with V6 before and after.
- **Full-bleed plus a max-width column double-nests** if a page wraps a `Band` in a
  `Container`. Guard by making `Band` own its container and never accepting one.
- **`svelte-check` is warning-free today.** Restructuring markup is the most likely way to
  introduce an a11y warning, which fails the build. Run `pnpm check` per commit, not per
  phase.
- **A gapless strip has no internal focus outline room.** Check focus-visible on adjacent
  coloured cards — the ring must not be clipped by the neighbour.
