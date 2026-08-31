# EasyQuran Design System

> **The pill / cobalt system** — version 2.0 · August 2026
> This document describes what is **in the code now**, not aspirations.
> Migration history and per-phase record: `docs/plan/README.md` (completed 2026-08-31).

---

## 0. Executive summary

The app moved off the v1 "Sacred Editorial" direction (forest/ivory/antique gold,
Newsreader + Inter, 10–18px radii, tinted grounds) onto a **pill / cobalt** system:

| Layer | Now |
| --- | --- |
| Ground | One **zero-chroma neutral ground** shared by every palette, per mode |
| Accents | 4 palettes: **sacred = cobalt**, **ink = neutral**, **sepia = magenta**, **sapphire = emerald** |
| Hue slots | 4 fixed hues (cobalt/emerald/purple/rose), identical across palettes, **mode-dependent** |
| Dark fills | **White-on-colour**: dark-mode accents are *darker* than light-mode ones, carrying white text |
| Typography | **Nunito** (self-hosted, variable) for UI + display; Noto Sans Arabic for Arabic UI; Amiri for the Quran column |
| Geometry | **Pill controls** (999px), 8px icon holders, 10px blocks, 14px large panels, 16px headline highlight |
| Layout | Full-bleed **Band** model; responsive ramp as a **utility ladder** (gutter 20/32/48/72, band pad 48/64/80/96) |
| Elevation | **None** — separation is borders and tonal surface steps, never shadows |
| Muted floor | `--muted` at L 0.52 (light) / 0.625 (dark) is contrast-derived — do not raise |
| Icons | Lucide line icons, 18/20/24; directional glyphs mirror automatically under RTL |

The Quran reading surface stays deliberately quiet: the reading column inherits the
neutral ground (only **sepia** keeps a warm reader in both modes), and the Quran text
is the highest-contrast element on the page in every palette and mode.

### Section anchors

Section numbers in this document are **stable anchors cited from source code**
(`web/src/**` comments and `web/src/routes/layout.css` section markers cite e.g.
"§4", "§61"). Numbers with no section here are v1 sections retired by the migration;
their content, where still relevant, lives in the nearest numbered section below or in
`docs/plan/README.md` and git history. Do not renumber.

---

## 1. Provenance and migration record

- v1.0 (this document's predecessor) specified "Sacred Editorial" and was implemented
  in an earlier run: token file `web/src/routes/layout.css`, primitives in
  `web/src/lib/components/ui/**`, palette contract tests.
- The 2026 migration (specs formerly `docs/plan/00`–`08`, now merged into
  `docs/plan/README.md`) moved that system to the pill/cobalt
  direction defined by two boards in the untracked `design/` directory:
  `PillLightCobalt.dc.html` (light) and `white-on-colour-deep-cobalt.html` (dark).
- Phase order (plan 08): visual harness → hue tokens inert → neutral ground →
  accents/`--accent` repoint → Nunito ramp → radius/primitives/state matrix →
  Band/RTL sweep → landing rebuild → app chrome/reader tokens → **this document**.
- Rollback notes, risk register, and per-phase commit discipline: plan 08.
  Decision record (palette id remap, white-on-colour, mode-dependent hues): plan 00.

Every machine guard landed with the change it guards and is enumerated in §61.

---

## 2. What changed from v1 (delta)

| v1 (Sacred Editorial) | v2 (pill/cobalt) |
| --- | --- |
| Per-palette tinted grounds | One shared **zero-chroma ground**; palette choice moves only the accent family |
| `sacred` = forest green | `sacred` = **cobalt** oklch(0.52 0.21 262) |
| `sepia` = parchment/umber | `sepia` = **magenta** 352 accent, warm *reader* kept |
| `sapphire` = navy/brass | `sapphire` = **emerald** 162 |
| `ink` = graphite accent | `ink` = **ground-inverted neutral** (white on near-black / near-black on white) |
| Editorial gold `--accent` | `--accent` = the **hue-2 family** (emerald); gold retired as a colour, name kept as alias |
| Newsreader display + Inter UI | **Nunito Variable** everywhere (self-hosted `@fontsource-variable/nunito`) |
| Radii 10–18px | **Pill 999** controls; 8/10/14/16 for holders/blocks/panels/highlight |
| Soft shadows | `--elev-sm/md = none`; borders + tonal surfaces |
| Fixed containers per page | Full-bleed **Band** + responsive utility ladder |
| Physical CSS directions | Logical properties everywhere (`ms-`/`pe-`/`start-`/`text-start`), machine-guarded |

---

## 3. Design principles

1. **The ground is neutral; colour is a decision.** All chrome sits on the same
   zero-chromatic grey scale per mode; hue appears only where it means something
   (primary action, hue slots, status).
2. **White-on-colour in the dark.** Dark fills are deep colour carrying white text —
   dark accents are *darker* than their light-mode counterparts (plan 00 D2), never
   pastel-on-dark.
3. **Pills for controls, blocks for content.** Anything you act with is a pill;
   anything you read sits in 8/10/14px blocks.
4. **Separation without shadow.** Hairline borders and surface luminance steps.
   The elevation tokens exist and resolve to `none`.
5. **Contrast is a gate, not a hope.** 96 token pairs across all 8 palette×mode
   blocks are asserted in `pnpm test` (§9).
6. **RTL is a first-class mode.** Logical properties; directional icons mirror in
   the icon component, not at call sites (§50).
7. **Semantic tokens only in components** (§61). No hex, no Tailwind default-palette
   classes on migrated surfaces.

---

## 4. Theme architecture — the semantic contract

Implemented in `web/src/routes/layout.css`; machine-guarded by
`web/src/routes/__tests__/palette-contract.test.ts`.

**Two independent attributes on `<html>`:**

```html
<html data-palette="sacred" data-mode="dark">
```

- `data-palette` ∈ `sacred | ink | sepia | sapphire` (§25)
- `data-mode` ∈ `light | dark` (resolved from the `light | dark | system` setting)

**Eight blocks** (`[data-palette="X"][data-mode="Y"]`) define the full contract.
`:root` doubles as the sacred-dark block so no-JS/SSR markup always resolves a
complete token set — every token must exist in `:root`.

**Contract tokens** (per block):

| Token | Role |
| --- | --- |
| `--background`, `--background-subtle` | Page ground and its one quiet step |
| `--surface`, `--surface-raised`, `--surface-hover` | Cards/surfaces, popovers/overlays, hover tint |
| `--foreground`, `--foreground-secondary`, `--muted` | Text ramp; `--muted` is text-side, never a surface (§9) |
| `--border`, `--border-strong` | Hairline and strong divider |
| `--primary`, `--primary-hover`, `--primary-foreground`, `--primary-soft` | Accent family (§5) |
| `--accent`, `--accent-strong`, `--accent-soft` | The **hue-2 family** (§8) |
| `--success`, `--warning`, `--danger` | Status — one set per mode, shared by all palettes |
| `--focus-ring` | Keyboard focus outline colour (= `--primary`) |
| `--reader-background`, `--quran-foreground`, `--translation-foreground`, `--reader-divider` | Reading surface (§42) |
| `--hue-1..4`, `--hue-N-soft`, `--on-hue-N`, `--hue-N-legible` | Hue slots (§6) |
| `--elev-sm`, `--elev-md` | Elevation — both `none` (§14) |

Neutral ground values (identical in every palette within a mode):

| Token | Light | Dark |
| --- | --- | --- |
| `--background` | `oklch(0.980 0 0)` | `oklch(0.165 0 0)` |
| `--background-subtle` | `oklch(0.968 0 0)` | `oklch(0.190 0 0)` |
| `--surface` / `--surface-raised` | `#ffffff` / `#ffffff` | `oklch(0.238 0 0)` / `oklch(0.262 0 0)` |
| `--surface-hover` | `oklch(0.953 0 0)` | `oklch(0.285 0 0)` |
| `--foreground` | `oklch(0.20 0 0)` | `oklch(0.965 0 0)` |
| `--foreground-secondary` | `oklch(0.40 0 0)` | `oklch(0.775 0 0)` |
| `--muted` | `oklch(0.52 0 0)` | `oklch(0.625 0 0)` |
| `--border` / `--border-strong` | `oklch(0.885 0 0)` / `oklch(0.82 0 0)` | `oklch(0.305 0 0)` / `oklch(0.375 0 0)` |

Custom themes (user colour seeds in prefs) derive through
`web/src/lib/theme/derive.ts`, which quantises a background seed to its
luminance-equal grey before ramping — custom grounds are also zero-chroma — and
derives an `--on-hue-1` for the accent seed. The utility layer (`@theme inline` in
layout.css) maps every contract token to its Tailwind colour utility
(`bg-background`, `text-foreground`, `border-border`, …), plus shadcn aliases and
legacy v1 aliases (§8) so un-migrated code keeps rendering.

---

## 5. Palettes and accent families

Palette ids are **remapped, never renamed** — prefs storage depends on them
(plan 00 D1; `prefs.test.ts` pins this).

| id | Accent | Light `--primary` | Dark `--primary` | Notes |
| --- | --- | --- | --- | --- |
| `sacred` | Cobalt | `oklch(0.52 0.21 262)` | `oklch(0.50 0.20 262)` | Default; `:root` block is sacred-dark |
| `ink` | Neutral | `oklch(0.20 0 0)` | — ground-inverted | On-accent = ground inversion (white on near-black light; near-black on white dark) |
| `sepia` | Magenta | `oklch(0.54 0.23 352)` | `oklch(0.50 0.23 352)` | Keeps a **warm reader** in both modes (§42) |
| `sapphire` | Emerald | `oklch(0.53 0.16 162)` | `oklch(0.50 0.14 162)` | Light value is the board's 0.54 nudged to clear the 4.5:1 gate |

- `--primary-hover` is ±0.04 L: darker in light mode, lighter in dark mode; the
  white-on-fill pair stays ≥4.5:1 in all eight blocks.
- `--primary-soft` is an opaque tint — light `oklch(0.95 0.045 H)`, dark
  `oklch(0.30 0.055 H)` (chroma 0 for ink).
- `--focus-ring` = `--primary` in every palette.
- **Dark accents are darker than light accents** — the white-on-colour decision.
  In dark mode a deep fill carries white text; a lighter fill would need dark text
  and read as a pastel chip, which this system does not do.

The picker swatch contract is `PaletteDef.accentHex { light, dark }` in
`web/src/lib/config/site.ts` (the shared ground made the old ground preview useless):
sacred `#1a5cdf`/`#1957d2`, ink `#161616`/`#f3f3f3`, sepia `#c7007c`/`#b90073`,
sapphire `#00864e`/`#007a49`. Display names live **only** in messages
(`settings_palette_*`, `tweaks_palette_*` — Cobalt/Ink/Magenta/Emerald, en+ar),
never in `site.ts`.

---

## 6. Hue slots

Sixteen additive tokens per block: `--hue-N`, `--hue-N-soft`, `--on-hue-N`,
`--hue-N-legible` for N = 1..4. The set is **identical across palettes** and
**mode-dependent**: dark values are the same L/C step-down the light ones use
(the retired plan 00 D3 amber→ember swap is gone — hue 4 is rose in both modes).

| Slot | Light | Dark | On-fill |
| --- | --- | --- | --- |
| `--hue-1` cobalt | `oklch(0.52 0.21 262)` | `oklch(0.50 0.20 262)` | white |
| `--hue-2` emerald | `oklch(0.53 0.16 162)` | `oklch(0.50 0.14 162)` | white |
| `--hue-3` purple | `oklch(0.50 0.24 300)` | `oklch(0.48 0.23 300)` | white |
| `--hue-4` rose | `oklch(0.52 0.19 25)` | `oklch(0.50 0.19 25)` | white |

- Softs: light `oklch(0.95 0.045 H)`-family tints; dark deep fills `oklch(0.30 0.05 H)`.
- `--hue-N-legible` is the colour for text/numerals *on* a soft: in light mode it is
  simply `--hue-N`; in dark mode it is an L≈0.78 version of the hue, because on the
  dark soft chips the deep fill disappears and the base hue would sink into it.
- Consumers: `MetricCard` (`hue={1|2|3|4}`), landing surah-number chips and badges,
  roadmap/steps accents. Consume via `var(--hue-N)` / `--hue-N-soft` /
  `--hue-N-legible` + `--on-hue-N` for on-fill pairs.

---

## 7. Zero-chroma ground

Every palette's ground within a mode is byte-identical and has chroma 0 (asserted by
the palette contract test via OKLCH parsing). Palette choice therefore moves only:
the primary family, the reader warmth (sepia), and nothing else. Consequences:

- Site chrome never re-tints per palette; a "palette" is an accent decision.
- Surface separation must come from borders and the surface ladder (§14), not tint.
- `--background-subtle` is the one quiet panel step; `Band tone="panel"` uses it.

---

## 8. Accent naming — `--color-accent` vs `--accent` (load-bearing)

Two similarly named things exist and both are depended upon. **Do not "fix" either.**

1. **`--accent` (custom property)** — the *secondary hue* role. Defined in every
   palette block as `var(--hue-2)` (emerald family): `--accent`, `--accent-strong`,
   `--accent-soft = --hue-2-soft`. The v1 editorial gold is retired; the meaning of
   this token is now "hue-2", and e.g. the Chip accent tone and marker dot resolve
   through it.
2. **`--color-accent` (Tailwind utility token, `@theme inline`)** — maps to
   `var(--primary)`. The `accent-*` *utilities* (`bg-accent`, `text-accent`, …) are
   **interactive/primary** colour for back-compat with shadcn-sourced components.
   `bg-accent` renders the palette's primary fill, identical to `bg-primary`.

So: `--accent` = hue-2 (secondary), `accent-*` utility = primary. A component using
`bg-accent` and a component using `var(--accent)` are referencing **different
colours** by design. The split is pre-existing (the utility mapping predates the
migration) and the migration made it load-bearing — retiring the utilities was
noted as a possible future cleanup (plan-06 judge round 1, nit 3) but has not been
done; until then this section is the contract.

Related legacy aliases (defined in `@theme inline`, kept so un-migrated code
renders): `gold/gold-strong/gold-soft` → the `--accent` (hue-2) family;
`pop/pop-soft` → same; `bg*/line*/fg*` → contract equivalents; `ok` → `--success`.
Remaining legal alias users: the `(account)` route, `design/` board variants, and
the UsageBar legend (`var(--pop-soft)`). New code uses contract names.

---

## 9. Contrast floors

Machine-gated by `web/src/routes/__tests__/token-contrast.test.ts` (96 pairs across
all 8 blocks; the shared evaluator also powers `node scripts/visual/contrast.ts`).

| Pair | Floor |
| --- | --- |
| `--foreground` / background | ≥ 7 |
| `--foreground-secondary` / background | ≥ 4.5 |
| `--muted` / background **and** / surface | ≥ 4.5 |
| `--primary-foreground` / `--primary` | ≥ 4.5 |
| Quran / translation on reader ground | ≥ 7 / ≥ 4.5 |
| borders / background | ≥ 1.1 (see below) |
| `--on-hue-N` / `--hue-N` | ≥ 4.5 (auto-activates per block) |

- The `--muted` lightness values (0.52 light / 0.625 dark) are the **contrast floor
  for 13.5px secondary text**, not cosmetics — never raise them.
- Border floor is 1.1:1, not a higher figure: the plan's own border tokens measure
  1.33:1 (light) and 1.44:1 (dark), so a 1.5 floor would contradict the tokens; the
  boards win. 1.1 still catches border==background and undefined-border bugs.
- Guard behaviour: an additive hue pair that is defined in *any* block but missing in
  another **fails** that block (no silent per-block skips).

---

## 10. Typography roles — fonts

All self-hosted via Fontsource (`web/src/routes/layout.css`); a CDN font reference
(`fonts.googleapis.com` / `fonts.gstatic.com`) is machine-banned — offline reading
depends on it.

| Stack | Token | Faces |
| --- | --- | --- |
| UI + display | `--font-sans` | **Nunito Variable**, Nunito, `ui-rounded`, system-ui, -apple-system, "Segoe UI", sans-serif |
| Arabic UI | `--font-arabic-ui` | Noto Sans Arabic (400/600), Segoe UI, Tahoma |
| Quran text | `--font-quran` | resolves the reader's runtime font choice; Amiri pre-hydration (`--font-arabic`: Amiri, Scheherazade New, Traditional Arabic, Geeza Pro) |
| Mono | `--font-mono` | Geist Mono Variable, ui-monospace, JetBrains Mono, SF Mono, Menlo |

- One Latin face: there is **no display face**; headings are Nunito 800 via the ramp.
  `--font-display`/`--font-serif`/Inter/Newsreader are machine-banned
  (`fonts.test.ts`).
- `ui-rounded` is the fallback chosen for character match — a slow font load
  degrades to something with the same feel.
- Arabic next to Latin: Amiri renders small next to Nunito at equal nominal size, so
  the boards run Arabic at ≈1.4×. `--font-size-arabic-ratio: 1.4`; the `.arabic`
  utility applies family + RTL + ratio (+ lh 2.1). `font-arabic` is family-only for
  fixed-size cases (the reading column has its own tuned sizing, out of scope here).
- The offline pack picks up the Nunito woff2 set (5 subsets) through the service
  worker's build manifest; Inter/Newsreader ship nowhere.

---

## 11. Type ramp

Sizes/line-heights/weights/tracking are one contract; tracking is part of the role,
not a per-use decision. Per-role weight and tracking ride the `text-*` utility — do
**not** add `font-semibold`/`tracking-*` next to a ramp class (an explicit `font-*`
utility wins; that is the override hatch, use it deliberately).

| Role | Size/line | Weight | Tracking |
| --- | --- | --- | --- |
| `text-display-xl` | 76px / 1.06 | 800 | −0.04em |
| `text-h1` | 40px / 1.1 | 800 | −0.035em |
| `text-h2` | 26px / 1.2 | 800 | −0.03em |
| `text-h3` | 20px / 1.25 | 800 | −0.025em |
| `text-body-xl` | 20px / 1.55 | 600 | — |
| `text-body-l` | 17.5px / 1.6 | 600 | — |
| `text-body` | 15px / 1.5 | 600 | — |
| `text-caption` | 13.5px / 1.45 | 600 | — |
| `text-micro` | 13px / 1.4 | 800 | +0.1em |

- `text-display-l` and `text-body-s` are retired roles kept as aliases of their
  nearest neighbours (h1 / body) for one release — prefer the real roles.
- Base `h1–h4` elements are weight 800; body element weight stays 400 so the Quran
  column (Amiri 400/700) never gets synthetic bold — ramp weights arrive via role
  classes.
- Board-only sizes (16.5/15.5/14.5 nav text, 44px metric numerals, 24px row Arabic,
  36px closing Bismillah) are not ramp roles — use arbitrary values or the nearest
  role, per the boards.
- Legacy `xs–4xl` utilities are unchanged; label uses the `body-s` role (alias of
  body).

---

## 12. Spacing

8px rhythm with a 4px micro-step, consumed through Tailwind's spacing scale and
`var()`. Band vertical rhythm and gutter are not free-form: they are the §15 ladder.

---

## 13. Radius and shape

| Token | Value | Use |
| --- | --- | --- |
| `--radius-sm` | 8px | Icon holders (the square behind an icon; metric numeral squares) |
| `--radius-md` | 10px | Blocks — rows, cards, panels, popovers, sheets, toolbars |
| `--radius-lg` | 10px | Same meaning as md (collapsed from 18 in v1); prefer `rounded-md` |
| `--radius-xl` | 14px | Large panels only (modals, toasts, side panels) |
| `--radius-pill` | 999px | Controls — buttons, inputs, tabs, chips, icon buttons |
| `--radius-highlight` | 16px | Headline highlight span (a pill at 76px becomes a lozenge and swallows the words) |

- A pill at 999px on any control height renders a true pill; that is the point.
- `rounded-full` remains **only** for genuinely round things: status/marker dots,
  spinners, blur orbs. (≈20 legacy pill-shaped `rounded-full` uses in application
  routes were swept to `rounded-pill` in plan 06.)
- Literal `rounded-[Npx]` is banned in `ui/` components (geometry guard); the one
  documented exception is Brand's 2px rotated diamond.

---

## 14. Elevation

`--elev-sm` and `--elev-md` are `none` in every block (dark mode keeps almost
nothing). The `shadow-sm`/`shadow-md` utilities resolve to none — do not add new
shadow usage. Separation = hairline borders + surface luminance steps
(background → subtle → surface → raised). Overlays keep a mode-invariant
`bg-black/55` scrim; that is a scrim, not elevation.

---

## 15. Responsive ramp — the Band ladder

The band model's gutter and vertical rhythm below the 1440px boards are designed
once, as the **utility ladder the `Band` component emits** — not custom properties:

| | base (<768) | md (768) | lg (1024) | xl (1280) |
| --- | --- | --- | --- | --- |
| Gutter | 20 (`px-5`) | 32 (`md:px-8`) | 48 (`lg:px-12`) | 72 (`xl:px-18`) |
| Band pad | 48 (`py-12`) | 64 (`md:py-16`) | 80 (`lg:py-20`) | 96 (`xl:py-24`) |
| Tight pad | 40 (`py-10`) | 48 (`md:py-12`) | 56 (`lg:py-14`) | 64 (`xl:py-16`) |

Rules:

- **Never re-introduce responsive custom props** for the ramp: the viteplus CSS
  stage deterministically drops or mangles `@media` rungs that re-declare ramp
  properties on `:root`/`html` (layout area round-1 major; six probe builds). The
  compiled-CSS guard bans it. Responsive values ride Tailwind's md/lg/xl variants.
- Pages consume `Band`'s `pad`/`width` props — they never invent breakpoints.
- A caller overriding the ladder must override **all its rungs** (Section does:
  `px-6 md:px-6 lg:px-6 xl:px-6`) — tailwind-merge resolves per-variant, so a bare
  `px-6` would leave the md/lg/xl rungs alive.
- The tight ramp stays below the default pad at every width.

---

## 16. Band, Section, Container

`web/src/lib/components/layout/Band.svelte` — the full-bleed primitive:

- `tone`: `page` (transparent — the page ground already is `--background`) |
  `panel` (`bg-background-subtle`) | `surface` (`bg-surface`) |
  `accent` (`bg-primary text-primary-foreground` — fill and on-fill text are an
  inseparable pair).
- `pad`: `default` | `tight` | `none` (the §15 ladders).
- `width`: `default` 1200 | `narrow` 880 | `wide` 1440 | `full` (gutter only, no
  cap). Boards' canvas = 1440 = `wide`.
- `rule`: hairline top border for where the tone does not change.
- `contentClass`: escape hatch for the inner column.
- A band owns its container: never wrap one in a `Container`, never pass one in.
  Bands never gain horizontal margin — a band with side margins is a card again.
- Separation between bands is the tone changing.

`Section` is the deprecated output-identical wrapper over `Band` (kept for one
release; landing pages port straight to `Band`). `Container` still exists for
non-band contexts. The gapless metric strip is a `pad="none" width="full"` band with
a `px-0`-ladder, `gap-0` grid (1 col <768, 2×2 md, 4 ≥1024) — gapless at every
width is a hard rule.

---

## 19. Badges (Chip, static form)

`web/src/lib/components/chip/Chip.svelte` without `onclick`. Sizing: 24px height,
8px inline padding, 11px/16 medium, **pill radius**. Tones (first match):

| Tone | Classes |
| --- | --- |
| `active` | `bg-primary text-primary-foreground` |
| `accent` | `bg-gold-soft text-gold-strong` — i.e. the `--accent` **hue-2** soft pair (§8); "Makki gold" is v1 vocabulary, the render is hue-2 |
| `ghost` | transparent, secondary text |
| default | `bg-surface` + `--border` hairline, secondary text |

Leading marker `dot`: 6px round dot in `--accent` (hue-2). Machine-guarded by
`chip.test.ts`.

---

## 21. Ayah interaction

Hover affordance on an ayah row is a **quiet surface tint** (`--surface-hover`),
transparent at rest, suppressed in continuous reading mode. State is never colour
alone (§51). The reading column itself (verse row structure, Arabic sizing, nav
logic) is out of scope for the design system by explicit fence (plan 00 D4) — it
inherits tokens only.

---

## 22. Bismillah / surah openers

Ceremonial but minimal: 42–48px Arabic, generous block margin (≈40–52px), no
ornament, no panel, no border — the same treatment in the surah and range readers.
The landing closing band renders a `lang="ar" dir="rtl"` Bismillah at 36px.

---

## 25. Reader settings and the palette/appearance model

- **Two separate settings** (stored in prefs, `web/src/lib/stores/prefs.svelte.ts`):
  - `palette` ∈ `sacred | ink | sepia | sapphire` (§5; ids are storage-frozen —
    remapped, never renamed)
  - `appearance` ∈ `light | dark | system`; `system` resolves via `matchMedia`;
    `mode` mirrors the resolved value. Back-compat migration: legacy `surface` →
    palette, `theme` → mode (`SURFACE_TO_PALETTE` in `site.ts`); explicit new
    fields win.
- The appearance pickers (settings `AppearanceSection`, reader `Tweaks`) render the
  **accent swatch** (`PaletteDef.accentHex`), rounded-sm — under the shared neutral
  ground a ground preview would show four identical grey chips.
- Palette display names live only in messages (`settings_palette_*` /
  `tweaks_palette_*`, en+ar: Cobalt/Ink/Magenta/Emerald).
- Reader typography controls (Arabic family/size) are unchanged by the design
  system; the reader inherits ground/hue tokens and only sepia warms (§42).

---

## 33. Filter chips (Chip, interactive form)

`Chip` with `onclick` renders a `<button type="button">`; state is exposed as
`aria-pressed`, never colour alone (§51). Geometry identical to §19. State matrix
(plan 03): hover = `--surface-hover` on the **inactive** chip only (no
`border-strong` — that is the row pattern), so the active primary fill is never
washed out by a hover surface; focus-visible = 2px `--focus-ring` outline, offset 2.

---

## 34. State matrix (all interactive primitives)

| Primitive | Hover | Active | Focus-visible | Disabled |
| --- | --- | --- | --- | --- |
| Button primary | `--primary-hover` | `translate-y-px` | 2px `--focus-ring` outline, offset 2 | `opacity-50 pointer-events-none` |
| Button secondary/ghost/quiet, chip, tab | `--surface-hover` (+`border-strong` on secondary only) | `translate-y-px` (buttons/icon-buttons) | same | same |
| Icon button | `--surface-hover` (ghost) | `translate-y-px` | same | same |
| Coloured card (MetricCard) | `brightness(1.08)` — no second fill per hue | — (non-interactive by design) | — | — |

Every button variant carries a hover state (guarded). Tab/mode-toggle active =
primary fill, with a `data-active:hover` override so hover never washes the fill.

---

## 35. Buttons

`web/src/lib/components/ui/button/` (§35 contract + plan 03 geometry/state matrix).
Base: `rounded-pill`. Sizes: sm `px-4` (compact, dense/inline contexts — the 44px
rule applies to core CTAs), md `px-6`, lg `px-7`. Variants: primary (accent fill +
`--primary-foreground`), secondary (border + `--surface-hover` hover), ghost, quiet
(gains a hover background), link. The legacy interactive-accent CTA maps onto the
primary role. Height floor 44px on core CTAs (§51). Semantic tokens only (§61).
Guarded by `primitives.test.ts`.

---

## 36. Icons and icon buttons

- One line family: **Lucide**. Sizes 18/20/24; stroke width via CSS (overrides the
  presentation attribute).
- Icon buttons (`ui/icon-button/`): **pill** radius at all sizes (999 on a square is
  the boards' circular toggle), 44px targets (§51), accessible name required —
  rendered as `aria-label`; state matrix per §34/§35. Ghost hover = `--surface-hover`.
- **Directional glyphs mirror under RTL inside `Icon.svelte`** (path-level
  `scale(-1,1)` translate; `arrow-right` is the only directional glyph). Call sites
  must NOT pass their own `rotate-180` for direction — that double-flips. Consumer
  rotations for other purposes (±90 chevrons) still compose.

---

## 37. Inputs

- `input.svelte`: 44px height, **pill radius**, `px-4` — controls are pills.
- `textarea.svelte`: the multiline variant keeps `rounded-md` (10px) — a multiline
  pill is a lozenge; blocks are for content.
- Labels use the `body-s` typography role (§11 alias).
- Semantic tokens only (§61); works across all 4 palettes × light/dark.

---

## 38. Tabs and segmented controls

Tabs are content destinations, rendered as **pill triggers** in a plain `gap-2` row
(the v1 hairline is gone). Active trigger = **primary fill** +
`--primary-foreground`; a `data-active:hover` override keeps hover from washing the
fill. 44px pill targets (§51). Mode toggles in reader chrome follow the same
active-fill pattern.

---

## 42. Reader surface

Reading-column tokens derive from the **neutral ground, not a tint**:

| Token | Light | Dark |
| --- | --- | --- |
| `--reader-background` | `#ffffff` | `oklch(0.190 0 0)` |
| `--quran-foreground` | `oklch(0.16 0 0)` | `oklch(0.975 0 0)` |
| `--translation-foreground` | `oklch(0.40 0 0)` | `oklch(0.775 0 0)` |
| `--reader-divider` | `oklch(0.905 0 0)` | `oklch(0.275 0 0)` |

- Dark reader ground sits just under `--surface` (0.190 < 0.238) so the column
  **recesses**; the Quran text stays the highest-contrast element on the page in
  both modes.
- **Sepia exception (dialled back)**: `sepia` keeps a *whisper* of warmth in its
  reader in **both** modes (light `#FAF8F3`, warm browns) under its magenta
  chrome — the original cream ground (`#FBF3DF`) read as a yellow tint and was
  near-neutralised (CSS-commented in layout.css). All other palettes are neutral
  in both modes.
- Status colours are one set per mode, shared by every palette (the ground is
  shared; per-palette status variants were drift): light `#397A57/#8A681E/#B34A4A`,
  dark `#78BD91/#D9B55E/#DF7777`. Status tokens are not in the contrast gate.

---

## 43. Colour accents and decoration

Decoration is rare and structural: hue-slot chips and badges (§6), the marker dot
(§19), the headline highlight span (16px radius, primary fill). No gradients, no
ornament, no drop shadows. Colour means something (action = primary, hue slot =
categorical, status = status) or it is not there.

---

## 50. Bidirectional / RTL layout rules

- Logical properties everywhere on migrated surfaces (machine-guarded by
  `logical-properties.test.ts` over layout + marketing): `ms-/me-/ps-/pe-`,
  `start-/end-`, `text-start`, `border-s/border-e`. Physical `ml-/mr-/pl-/pr-/left-N/
  right-N/text-left|right` are banned in guarded trees.
- On a `dir="rtl"` element logical axes resolve against that element — `ml-auto` on
  an RTL span is a right-push; use the logical property that keeps the *visual*
  intent (the GlobalSearchPalette bilingual row is the worked example: outer span
  `ms-auto` inheriting row direction, inner `dir="rtl"` span for shaping).
- **Stays physical, deliberately**: `left-1/2 -translate-x-1/2` centering (logical
  insets break centering under RTL) and `data-[side=left|right]:…` (side = physical
  screen edge).
- Directional icon glyphs mirror inside `Icon.svelte` (§36).
- Locale switches are full navigations; per-render locale reads are sufficient.

---

## 51. Accessibility baseline

- **44px targets** on core interactive controls (buttons, tabs, inputs, icon
  buttons); sm stays compact for dense table/inline contexts (documented carve-out).
- Icon-only controls must carry an accessible name (`aria-label`).
- Interactive state is exposed semantically (`aria-pressed`), never colour alone.
- `focus-visible` everywhere: 2px `--focus-ring` outline, 2px offset — never remove
  without a replacement.
- Contrast per §9 is a gate, including hue pairs and all 8 blocks.
- Reduced-motion, screen-reader and keyboard paths are preserved by the primitives;
  overlays are dismissible and scrimmed (§14).

---

## 59. The direction, in one page

A neutral, near-grey canvas at every palette; one decisive accent per palette
(cobalt / neutral / magenta / emerald) plus four fixed hue slots for categorical
colour; deep dark fills that carry white; Nunito at 800 for display and 600 for
body; pill controls on 8/10/14px blocks; full-bleed bands on a 20/32/48/72 gutter
ladder; no shadows; hairlines and tonal steps; logical properties for both
directions; and a reading surface that stays quieter than the chrome around it —
except in sepia, where the page you read is warm on purpose.

If code and this document disagree, **the code and its guards win**; fix the
document (or file the bug) — never edit prose to override a gate.

---

## 61. Agent brief — rules with teeth

For anyone (human or AI) changing UI code:

1. **Semantic tokens only.** No hard-coded colours (hex/rgb/hsl/oklch) in component
   code — colour literals live in `layout.css` token blocks and `site.ts` swatch
   data only. No Tailwind default-palette classes on migrated surfaces. Guarded by
   `primitives.test.ts` (§61 suite), `landing-guard.test.ts`, `geometry.test.ts`.
2. **Never rename palette ids** (`sacred/ink/sepia/sapphire`) — prefs storage.
   Guarded by `prefs.test.ts`.
3. **Never raise `--muted`** past 0.52/0.625 — contrast floor (§9). Guarded by
   `token-contrast.test.ts`.
4. **No CDN fonts**, no `--font-display`/`--font-serif`/Inter/Newsreader.
   Guarded by `fonts.test.ts`.
5. **Controls are pills**; `rounded-full` only for dots/spinners/orbs; no
   `rounded-[Npx]` literals in `ui/`. Guarded by `geometry.test.ts`.
6. **Ramp weight/tracking ride the `text-*` role** — no `font-semibold`/`tracking-*`
   beside a ramp class (§11).
7. **Band owns its container** — never `Container` inside a band; the metric strip
   stays gapless at every width; override the whole §15 ladder or none of it.
   Guarded by `logical-properties.test.ts` (band model + compiled-CSS ladder +
   logical-props bans).
8. **Reader fence**: reading column layout, verse rows, Arabic font stack, page/juz
   nav logic are out of scope — reader inherits tokens; sepia warms (§42).
9. **Reader nav hrefs** use the `surah*For(ctx,…)` family only; never hand-built
   `/app/` strings. Guarded by `nav-guard.test.ts`.
10. **No nested ternaries** anywhere; lookup tables or early-return functions
    (repo lint rule).
11. **Keyboard shortcuts** go through `registerHotkey()` in
    `web/src/lib/hotkeys.svelte.ts`, dynamically imported (repo rule).
12. **No responsive custom props for the ramp** (§15) — the compiled-css guard fails
    the build.
13. Gates that must stay green untouched: `nav-guard`, `prefs`, `reader-fonts`,
    `settings-document`, `surah-reader`, `VerseRow.stacking`, `page-heights`,
    `catalogue-sha-guard`, `route-isolation`, `usage-bar-guard`,
    `auth-form-focus`.
14. Three gates green before every commit: `pnpm check`, `pnpm lint`, `pnpm test`
    (run in `web/`; if lint reports TS2307 on `server.ts`, run `pnpm build` first —
    the adapter output is a gitignored build artifact).

Visual verification harness: `web/scripts/visual/` (capture / states / contrast /
reference; matrix = 4 palettes × 2 modes × 5 widths). Token sweep host lives at
`/design/tokens`. Plan 07 defines the V1–V11 checks.
