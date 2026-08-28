# 00 — Scope and decisions

## Goal

Settle the questions that change the shape of the work, before any code moves.

---

## D1. What happens to the four palettes?

**Current.** `PaletteId = "sacred" | "ink" | "sepia" | "sapphire"`
(`web/src/lib/config/site.ts:55`). Each has a light and a dark block in
`web/src/routes/layout.css` — eight blocks, all guarded by
`routes/__tests__/palette-contract.test.ts`. The id is persisted in user prefs, mirrored
into the legacy `surface` field via `PALETTE_TO_SURFACE`, and decoded by
`lib/settings/settings-document.ts` — which carries a "frozen this round" comment and is
consumed by the sync engine.

**The new design has one ground and a choice of accent.** That is a different axis from
the old palettes.

### Options

| | Approach | Cost |
| --- | --- | --- |
| **A** | Rename the ids to accent names (`cobalt`, `magenta`, …) | Breaks persisted prefs, `SURFACE_TO_PALETTE`, `PALETTE_TO_SURFACE`, the settings document decoder, and anything already synced to a server. Needs a migration path and a decoder version bump. |
| **B** *(recommended)* | **Keep the four ids as opaque storage keys; redefine what they render.** | Zero data migration. `settings-document.ts` stays frozen. Only `layout.css` and the palette preview swatches change. |
| **C** | Collapse to a single palette, drop the setting | Removes a shipped user-facing feature; the appearance UI and its tests would need unpicking. |

### Decision: **B**

The id is already an opaque token in storage — nothing outside `layout.css` and the
swatch previews knows what "sepia" looks like. Remap:

| Stored id | Was | Becomes |
| --- | --- | --- |
| `sacred` | forest / ivory / gold | **Cobalt** — the canonical accent, stays `DEFAULT_PALETTE` |
| `ink` | strict black / white | **Neutral** — accent is the neutral ink itself, no hue |
| `sepia` | parchment / umber | **Magenta** |
| `sapphire` | slate / navy / brass | **Emerald** |

Two follow-ons:

- `PaletteDef.lightHex` / `darkHex` (`site.ts:63`) are preview swatches equal to the
  palette's `--background`. Under the new system every palette shares one neutral
  background, so the swatches would all look identical. **Add an `accentHex` field** and
  render the swatch from the accent, not the ground. This touches the appearance UI in
  plan 06.
- User-visible palette *names* live in i18n, not in `site.ts`. Renaming the displayed
  label is a copy change in `web/messages/{en,ar}.json`, subject to `pnpm i18n:check`.

> **Open for the owner:** whether `ink` keeps a neutral, hueless accent (recommended — it
> is the accessibility-safe high-contrast option) or becomes a fourth hue.

---

## D2. Deep or mid lightness for the dark coloured surfaces?

The dark boards were produced at two levels:

| Level | Fill lightness | White text contrast | Verdict |
| --- | --- | --- | --- |
| **Deep** | L ≈ 0.50 | ≈ 4.5:1 | Passes for the 13.5px sub-lines under each metric |
| **Mid** | L ≈ 0.58 | ≈ 3.5:1 | Fails normal-size text; fine for numerals, headings, buttons |

`design/white-on-colour-deep-cobalt.html` — the file kept in `design/` — is **deep**.

### Decision: **deep**, with one exception path

Ship deep. If the owner wants mid's richness, it can only apply to surfaces whose
smallest text is ≥ 18.66px bold or ≥ 24px regular; the metric cards' sub-line would have
to be dropped or restyled first. Do not mix levels within one board.

---

## D3. Do amber and lime survive?

No — not on the dark ground. No yellow bright enough to read as amber can carry white
text at any lightness. The dark board substitutes **ember** (warm orange, L ≈ 0.53) and
**forest** (L ≈ 0.50).

The light board keeps amber and lime, where they take near-black text
(`--on-hue-*` handles this; see plan 01). So the hue set is **mode-dependent**, which is
a genuine complication the token layer has to model rather than paper over.

---

## D4. How far does the full-bleed treatment go?

The mockups only cover the **landing page**. Applying full-bleed bands to the reader
would be a much larger change to a surface that is already tuned for long-form reading.

### Decision: staged

- **Phase 1–5** (plans 01–05): foundations, primitives and the marketing surface adopt
  the new system fully.
- **Phase 6** (plan 06): app chrome — nav, header, footer, settings — adopts the tokens,
  the type ramp and the geometry, but **keeps its current layout**. The reader's reading
  column is explicitly out of scope.
- A later, separate plan can revisit reader layout once the new system has shipped.

---

## D5. Is Nunito the final face?

It is what both boards use, and it was chosen over Space Grotesk, Plus Jakarta Sans,
Instrument Sans and Onest in a direct comparison. Treat it as settled.

The constraint that matters is not aesthetic: **fonts must be self-hosted**
(`AGENTS.MD`). `@fontsource-variable/nunito` must exist and be added as a dependency;
the mockups' `fonts.googleapis.com` link must never reach the app. Plan 02.

---

## Out of scope

- Quran text rendering, the Arabic font stack, and `lib/config/reader-fonts.ts`.
- Reader layout, verse row structure, page/juz navigation.
- Any data, sync, auth or offline behaviour.
- Hover, focus and active states are **not** drawn in the mockups. They must be designed
  during implementation, not invented per-component — plan 03 owns the state matrix.
- RTL is not modelled in the mockups (they use `margin-left`, not logical properties).
  Every ported style must use logical properties — plan 04.
- Responsive behaviour. The boards are fixed at 1440px. Breakpoint behaviour is designed
  in plan 04 and verified in plan 07.

## Definition of done for the whole migration

1. `pnpm check`, `pnpm lint`, `pnpm test` green.
2. No `fonts.googleapis.com` reference anywhere in `web/`.
3. No hard-coded hex or Tailwind palette class in any component (the §61 guard, extended).
4. The landing page at 1440px matches `design/PillLightCobalt.dc.html` on the checks in
   plan 07, in both modes and all four palettes.
5. Contrast pairs verified by the automated check in plan 07 — not by eye.
6. `docs/design-system.md` updated or superseded, so the repo has one design truth.
