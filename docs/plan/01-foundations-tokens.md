# 01 — Foundations: the token contract

## Goal

Replace the four tinted palettes in `web/src/routes/layout.css` with a zero-chroma
neutral ground plus a vibrant accent and a four-hue card set, without breaking the
semantic contract that 92 components already consume.

## Current state

`web/src/routes/layout.css` (732 lines) is in four parts:

| Lines | Part |
| --- | --- |
| 1–11 | Tailwind + font imports, `@custom-variant dark` keyed off `[data-mode="dark"]` |
| 13–88 | `@theme` — font families, type scale, radii, breakpoints, easings, animations |
| 89–172 | `@theme inline` — semantic contract → Tailwind utilities, plus legacy aliases |
| 173–480 | Eight `[data-palette][data-mode]` blocks, each defining 25 contract tokens |
| 481–732 | Elevation, the legacy runtime shim, `@layer base` |

The 25 contract tokens are enumerated in `routes/__tests__/palette-contract.test.ts`.
Every one must be present in all eight blocks or the build fails.

`:root` doubles as the `sacred`-dark default so SSR and no-JS markup always resolve a
complete contract (`layout.css:176`).

## Target state

### The ground goes neutral

Every palette shares one ground. Only the accent and hue set differ.

```css
/* light — identical across all four palettes */
--background:         oklch(0.980 0 0);
--background-subtle:  oklch(0.968 0 0);
--surface:            #ffffff;
--surface-raised:     #ffffff;
--surface-hover:      oklch(0.953 0 0);
--foreground:         oklch(0.20 0 0);
--foreground-secondary: oklch(0.40 0 0);
--muted:              oklch(0.52 0 0);
--border:             oklch(0.885 0 0);
--border-strong:      oklch(0.82 0 0);

/* dark — identical across all four palettes */
--background:         oklch(0.165 0 0);
--background-subtle:  oklch(0.190 0 0);
--surface:            oklch(0.238 0 0);
--surface-raised:     oklch(0.262 0 0);
--surface-hover:      oklch(0.285 0 0);
--foreground:         oklch(0.965 0 0);
--foreground-secondary: oklch(0.775 0 0);
--muted:              oklch(0.625 0 0);
--border:             oklch(0.305 0 0);
--border-strong:      oklch(0.375 0 0);
```

Values are lifted from the boards. `--muted` at 0.52 (light) / 0.625 (dark) is not
cosmetic — it is the lowest lightness that keeps 13.5px secondary text above 4.5:1 on the
respective ground. Do not raise it.

### The accent per palette

| Palette id | Light accent | Dark accent | On-accent |
| --- | --- | --- | --- |
| `sacred` (cobalt) | `oklch(0.52 0.21 262)` | `oklch(0.50 0.20 262)` | `#ffffff` |
| `sepia` (magenta) | `oklch(0.54 0.23 352)` | `oklch(0.51 0.22 352)` | `#ffffff` |
| `sapphire` (emerald) | `oklch(0.54 0.16 162)` | `oklch(0.50 0.14 162)` | `#ffffff` |
| `ink` (neutral) | `oklch(0.20 0 0)` | `oklch(0.965 0 0)` | ground-inverted |

Note the dark accents are **darker** than the light ones, not lighter. That is the
white-on-colour decision from plan 00 D2 and it is counter-intuitive enough to be worth a
comment in the CSS.

### New tokens: the hue set

The contract has no vocabulary for "four differently-coloured cards". Add it. These are
**additive** — the palette-contract guard checks presence, not absence, so nothing breaks.

```css
--hue-1: …;  --hue-1-soft: …;  --on-hue-1: …;
--hue-2: …;  --hue-2-soft: …;  --on-hue-2: …;
--hue-3: …;  --hue-3-soft: …;  --on-hue-3: …;
--hue-4: …;  --hue-4-soft: …;  --on-hue-4: …;
```

- `--hue-N` — the fill.
- `--hue-N-soft` — the tint behind a numeral chip. Light: L ≈ 0.95, chroma ≈ 0.045.
  Dark: L ≈ 0.30, chroma ≈ 0.055.
- `--on-hue-N` — **the foreground for text on that fill.** This token exists because amber
  and lime need near-black while every other hue needs white. Never assume white.

Cobalt, light:

```css
--hue-1: oklch(0.52 0.21 262);  --hue-1-soft: oklch(0.950 0.045 262);  --on-hue-1: #ffffff;
--hue-2: oklch(0.54 0.16 162);  --hue-2-soft: oklch(0.950 0.045 162);  --on-hue-2: #ffffff;
--hue-3: oklch(0.50 0.24 300);  --hue-3-soft: oklch(0.950 0.045 300);  --on-hue-3: #ffffff;
--hue-4: oklch(0.78 0.16 78);   --hue-4-soft: oklch(0.955 0.055 78);   --on-hue-4: oklch(0.20 0 0);
```

Cobalt, dark — amber becomes ember so white works (plan 00 D3):

```css
--hue-1: oklch(0.50 0.20 262);  --hue-1-soft: oklch(0.30 0.055 262);  --on-hue-1: #ffffff;
--hue-2: oklch(0.50 0.14 162);  --hue-2-soft: oklch(0.30 0.050 162);  --on-hue-2: #ffffff;
--hue-3: oklch(0.48 0.23 300);  --hue-3-soft: oklch(0.30 0.060 300);  --on-hue-3: #ffffff;
--hue-4: oklch(0.53 0.16 55);   --hue-4-soft: oklch(0.31 0.055 55);   --on-hue-4: #ffffff;
```

Plus one more, for numerals sitting *on* a dark soft chip, where a deep fill colour
disappears:

```css
--hue-N-legible: /* L 0.78 version of hue N, dark mode only; equals --hue-N in light */
```

### What happens to `--accent` (the gold family)

The contract requires `--accent`, `--accent-strong`, `--accent-soft` and the guard
asserts `--color-pop: var(--accent)`. The new system has no editorial gold.

**Map `--accent` onto `--hue-2`** (the second card hue) rather than deleting it. Every
existing `gold-*` / `pop-*` consumer keeps rendering, in a colour that belongs to the new
system. Add a comment saying the editorial-gold role is retired and `--accent` now means
"the secondary hue".

### Elevation

The boards have **no shadows at all**. `--elev-sm` / `--elev-md` (`layout.css:481`) become
`none` in both modes. Keep the tokens — `--shadow-sm` / `--shadow-md` utilities are
consumed elsewhere and removing them is a separate cleanup.

## Steps

1. **Add the hue tokens to all eight blocks** without touching anything else. Everything
   still renders as before; the new tokens are simply unused. Land and verify green.
2. **Swap the ground to neutral** in all eight blocks, one palette per commit. After each,
   run the visual check from plan 07 — the ground changing under existing components is
   the highest-risk moment in the whole migration.
3. **Swap the accents.** `--primary`, `--primary-hover`, `--primary-foreground`,
   `--primary-soft`, `--focus-ring` per palette.
4. **Repoint `--accent`** to the hue-2 family, with the comment.
5. **Zero the elevation tokens.**
6. **Update `theme/derive.ts`.** The custom-seed deriver
   (`web/src/lib/theme/derive.ts`) generates a full contract from a background/accent/pop
   hex. It currently emits warm-shifted ramps. It must now emit neutral ramps and derive
   `--on-hue-*` by luminance rather than assuming white — `isLight()` already exists at
   `derive.ts:41` and is the right predicate. `backgroundTokens()` keeps its shape;
   `accentTokens()` gains `--on-hue-*` output.
7. **Update the guards** (below).

## Verification

### Machine

- `pnpm test` — `palette-contract.test.ts` extended with:
  - all four `--hue-N`, `--hue-N-soft`, `--on-hue-N` present in all eight blocks;
  - the ground tokens are **identical across palettes within a mode** (this is the check
    that catches a stray tint sneaking back in);
  - `--background` in every light block parses as zero-chroma.
- `derive.test.ts` extended: a mid-grey seed produces zero-chroma ramps; a bright-yellow
  accent seed produces a **dark** `--on-hue-1`, not white.
- New guard `token-contrast.test.ts` — plan 07 owns it. It computes contrast for every
  `(--hue-N, --on-hue-N)` and `(--primary, --primary-foreground)` pair in all eight blocks
  and fails below 4.5:1.

### Visual

Plan 07, checks **V1** (token sweep) and **V2** (palette matrix). The token sweep renders
a page of every semantic token as a swatch, in all eight combinations — it catches a
missing or malformed token far faster than looking at real pages.

## Risks

- **The ground change is global and instant.** Every one of the 92 components sits on it.
  Mitigate by landing the hue tokens first (step 1, inert), then one palette at a time
  with a visual check between each.
- **`:root` is the SSR default.** If `:root` and `[data-palette="sacred"][data-mode="dark"]`
  drift apart, no-JS and pre-hydration markup renders a different theme than the app. The
  guard asserts they are one block — keep it that way.
- **`--accent` repointing is a silent visual change** anywhere `gold-*` is used. Grep for
  `gold-`, `pop-` and `accent-soft` consumers before step 4 and list them in the commit.
- **Contrast regressions are invisible in review.** The automated pair check is not
  optional; a reviewer cannot eyeball 4.5:1.
