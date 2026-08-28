# 07 — Visual verification

## Goal

Make "does it look right" a check that runs, not a judgement made once by whoever last
looked at the page. Every sub-plan cites a check number from this file.

## Why this needs building

The three existing gates prove the app compiles, lints and behaves. **None of them can
see.** A migration whose entire purpose is visual can pass all three while shipping
unreadable text, a clipped band, or a palette that never got its ground swapped.

Two failure classes matter most, and neither is caught by review:

1. **Contrast.** No reviewer can eyeball 4.5:1. It must be computed.
2. **Drift between palettes and modes.** Eight combinations × every surface is more than
   anyone checks by hand, so in practice only the default gets looked at.

## The harness

### Browser

Headless Puppeteer against a **throwaway profile** — never the developer's own Chrome, and
never the Claude-in-Chrome extension. Add as a `devDependency`; keep it out of the runtime
bundle.

```
web/scripts/visual/
  capture.ts        # launches, navigates, screenshots a matrix of routes × palette × mode × width
  contrast.ts       # parses layout.css, computes every token pair, fails below threshold
  reference.ts      # renders design/*.html in the same browser for side-by-side capture
  states.ts         # drives hover / focus-visible / active for the primitive board
  matrix.ts         # the shared list of routes, palettes, modes, widths
```

Output to `web/.visual/` (gitignored): `<check>/<route>__<palette>__<mode>__<width>.png`.

### Reuse what exists

`web/src/routes/design/` already contains a variants playground with
`design/landing/[variant]` and `design/reader/[variant]` routes and a
`_variants/registry.ts`. **Use it as the host for the synthetic check pages** (V1, V3, V5)
rather than inventing a parallel mechanism. It is already excluded from the marketing and
app route groups, and it already has a test (`_variants/__tests__/verses.test.ts`).

### Setting palette and mode without clicking

`prefs.svelte.ts:211` applies the theme by writing
`[data-palette="…"][data-mode="…"]` on the document. The harness sets those attributes
directly before capture — no UI driving, no flake:

```ts
await page.evaluate(([p, m]) => {
  document.documentElement.dataset.palette = p;
  document.documentElement.dataset.mode = m;
}, [palette, mode]);
```

Wait on `document.fonts.ready` before every screenshot, or Nunito's load races the capture
and every diff is noise.

### The matrix

| Axis | Values |
| --- | --- |
| Palette | `sacred`, `ink`, `sepia`, `sapphire` |
| Mode | `light`, `dark` |
| Width | 1440, 1280, 1024, 768, 390 |

Full cross-product is 40 captures per route — too slow for every commit. Tiered:

- **Per commit:** default palette, both modes, 1440 + 390.
- **Per phase:** all four palettes, both modes, 1440.
- **Before merge:** the full matrix.

## The checks

| # | Check | Owner plan | What it proves |
| --- | --- | --- | --- |
| **V1** | Token sweep | 01 | Every semantic token renders as a labelled swatch in all 8 combinations. Catches a missing or malformed token instantly. |
| **V2** | Palette matrix | 01 | Ground tokens are byte-identical across palettes within a mode; only accent and hues differ. |
| **V3** | Type specimen | 02 | Every ramp role, both weights, Latin and Arabic side by side, at true size. |
| **V4** | Offline fonts | 02 | After one visit, with the network offline, no text falls back to a system face. |
| **V5** | Primitive board | 03 | Every primitive × variant × **state** × mode. States are driven, not hoped for. |
| **V6** | Breakpoint sweep | 04 | No horizontal scrollbar, no clipped band, metric strip column count per the ramp. |
| **V7** | RTL mirror | 04 | The Arabic locale mirrors; anything not mirror-symmetric is a bug. |
| **V8** | Landing diff | 05 | `/` against `design/PillLightCobalt.dc.html`, section by section. |
| **V9** | Marketing sweep | 05 | The other five marketing pages, three widths, both modes. |
| **V10** | Reader inheritance | 06 | The reader before/after the ground change — human judgement, on real text. |
| **V11** | App sweep | 06 | Every app route including the surfaces that rarely render. |

### V1 — token sweep (build this first)

A page under `routes/design/` rendering every token in the §4 contract plus the new hue
set as a labelled swatch, with its computed value printed beside it. Capture in all eight
combinations.

This is the cheapest check with the highest yield: a token that failed to get defined in
one of eight blocks shows up as a wrong-coloured square, immediately, instead of surfacing
weeks later on one page in one palette.

### V5 — states must be driven

A screenshot captures rest state only. The harness must explicitly:

```ts
await el.hover();                                    // hover
await page.keyboard.press("Tab");                    // focus-visible
await page.mouse.down();                             // active
```

Focus-visible in particular only appears via keyboard, never via `.focus()`. A board that
looks complete but was captured with `.focus()` proves nothing.

### V8 — comparing against the reference

`design/PillLightCobalt.dc.html` is a Design Component file — strip the `<x-dc>` wrapper
and inline the `<helmet>` contents into `<head>` before loading it. The dark file is
already plain HTML.

**Do not pixel-diff the whole page.** The reference uses CDN fonts and fixed 1440px; the
app self-hosts and is fluid. Sub-pixel differences would fail every run. Compare instead:

1. **Section geometry** — bounding boxes of each band: y-offset, height, inner column
   left/right edges. Assert within ±2px.
2. **Computed styles at named anchors** — for the h1, the search field, a metric card, a
   surah row, the closing CTA: `font-size`, `font-weight`, `letter-spacing`,
   `border-radius`, `background-color`, `color`. Assert exact.
3. **Side-by-side capture** for a human to look at once per phase — not an automated gate.

This is a structural comparison, which is what actually matters, and it does not go red
because a font hinted differently.

## The contrast gate

`web/scripts/visual/contrast.ts`, wired into `pnpm test` as `token-contrast.test.ts`.

Parse `layout.css`, and for every one of the eight palette blocks compute WCAG contrast
for:

| Pair | Threshold |
| --- | --- |
| `--foreground` on `--background` | 7:1 (AAA — this is a reading product) |
| `--foreground-secondary` on `--background` | 4.5:1 |
| `--muted` on `--background` | 4.5:1 |
| `--muted` on `--surface` | 4.5:1 |
| `--primary-foreground` on `--primary` | 4.5:1 |
| `--on-hue-N` on `--hue-N`, N = 1…4 | 4.5:1 |
| `--quran-foreground` on `--reader-background` | 7:1 |
| `--translation-foreground` on `--reader-background` | 4.5:1 |
| `--border` on `--background` | 1.5:1 (visible, not a contrast requirement) |

The values are `oklch()`; `lib/theme/derive.ts` already has `luminance()` and `parseHex()`
but only for hex. Either extend it with an oklch parser or convert at the token source.
Extending `derive.ts` is preferable — it keeps one colour implementation and the existing
`derive.test.ts` covers it.

**This gate is the single highest-value artefact in the plan.** It converts the migration's
main risk from "someone notices later" into a build failure.

## Reporting

`capture.ts --report` emits `web/.visual/index.html` — a contact sheet of the current run,
grouped by check, with the reference beside the app for V8. One file to open, one page to
scroll, rather than a directory of PNGs.

## Definition of done

- V1–V11 all runnable by a single command each.
- The contrast gate is part of `pnpm test`.
- The full matrix has been run and reviewed once before merge.
- V10 has been done by a human, reading actual Quran text, in both modes.
