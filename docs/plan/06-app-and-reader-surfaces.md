# 06 — App and reader surfaces

## Goal

Bring the application chrome onto the new tokens, type and geometry **without**
restructuring the reader, and update the appearance settings so the palette picker still
makes sense when every palette shares a ground.

## Scope boundary

| In | Out |
| --- | --- |
| Nav, header, side panel, command palette | The reading column's layout |
| Settings, account, bookmarks, search pages | Verse row structure, ayah markers |
| Buttons, inputs, chips, tabs inside the app | Arabic/Quran font stack and sizing |
| Reader *chrome* — toolbar, controls, sheets | Page/juz navigation logic and hrefs |

The reader's reading surface is deliberately excluded (plan 00 D4). It is tuned for
long-form reading, has its own token family (`--reader-background`,
`--quran-foreground`, `--translation-foreground`, `--reader-divider`), and changing it is
a separate design exercise with its own verification needs.

What the reader **does** inherit automatically: the neutral ground, the new `--muted`, the
radius scale, and Nunito for its Latin UI text. That inheritance alone changes how the
reader looks and must be checked (V10 below) even though no reader file is edited.

## Current state

- 47 `.svelte` files under `routes/(application)`, 92 under `lib/components`.
- Reader tokens are defined per palette in each of the eight blocks
  (`layout.css`, `--reader-background` and siblings).
- Appearance settings render palette swatches from `PaletteDef.lightHex` / `darkHex`
  (`lib/config/site.ts:63`), which equal each palette's `--background`.
- `prefs.svelte.ts:211` applies the theme by writing
  `[data-palette="…"][data-mode="…"]` onto the document.

## Target state

### Reader tokens under a neutral ground

The reader keeps its four tokens, but they now derive from the neutral ground rather than
a tinted one:

| Token | Light | Dark |
| --- | --- | --- |
| `--reader-background` | `#ffffff` — a touch brighter than the page, so the column reads as paper | `oklch(0.190 0 0)` — a touch darker than `--surface`, so it recedes |
| `--quran-foreground` | `oklch(0.16 0 0)` | `oklch(0.975 0 0)` |
| `--translation-foreground` | `oklch(0.40 0 0)` | `oklch(0.775 0 0)` |
| `--reader-divider` | `oklch(0.905 0 0)` | `oklch(0.275 0 0)` |

The Quran text is the highest-contrast element on the page in both modes. That is
intentional and should not be softened for aesthetic balance.

**Sepia is the open question.** Under decision D1, the `sepia` id becomes magenta and its
warm parchment reading surface disappears. A warm reading ground is a genuine reading-
comfort feature, not decoration.

> **Open for the owner:** either (a) keep one palette with a warm `--reader-background`
> while its chrome stays neutral, or (b) add a separate reader-warmth setting orthogonal
> to palette. (b) is more work but stops the accent choice and the reading comfort choice
> from being welded together.

### The palette picker needs a new swatch

With one shared ground, four background swatches look identical. Change the swatch to show
the **accent**, and add `accentHex` to `PaletteDef` (plan 00 D1). The picker's label copy
also changes — "Sacred / Ink / Sepia / Sapphire" no longer describes anything. New labels
are i18n keys in `messages/{en,ar}.json`.

### Nav, panel and command palette

These are the surfaces where the geometry change is most visible:

- Nav items and the panel's controls → pill.
- The command palette (`lib/components/search/GlobalSearchPalette.svelte`) → 10px block,
  pill input, 8px square icon holders in results.
- Sheets and popovers → `--radius-xl` (14px), not the old 24px.
- Any icon holder currently `rounded-full` → `rounded-sm` per the plan-03 audit.

## Steps

1. **Reader tokens** in all eight palette blocks. No component edits. Verify with V10 —
   this is the change most likely to be judged by feel rather than by diff.
2. **Resolve the sepia/warmth question** before step 1 lands, since it determines whether
   one block keeps a warm reader ground.
3. **`PaletteDef.accentHex`** + the appearance picker + its i18n labels.
4. **Chrome sweep** — nav, header, footer, panel, command palette — one commit per
   surface.
5. **App pages** — settings, account, bookmarks, search — one commit each.
6. **Reader chrome only** — toolbar, font-size control, translation picker sheets.

## Verification

### Machine

- `prefs.test.ts` — unchanged and must stay green; it is the proof that palette ids were
  not renamed (plan 00 D1).
- `settings-document.test.ts` — unchanged; proves the frozen decoder still decodes.
- `usage-bar-guard.test.ts`, `route-isolation.test.ts` (settings and search) — these guard
  route boundaries, not styling, and must stay green untouched.
- `surah-reader.test.ts`, `VerseRow.stacking.test.ts`, `page-heights.test.ts` — reader
  behaviour guards. **If any of these fail, a reader change leaked out of scope.** That is
  the signal to stop and revert, not to update the test.

### Visual

Plan 07 checks **V10** (reader inheritance) and **V11** (app sweep):

- **V10** — the reader at a fixed surah/ayah range, all four palettes × both modes,
  before and after the token change. Read the Arabic at 100% and at the largest reader
  size; check the divider is visible but not loud, and that translation text clears 4.5:1.
- **V11** — every app route at 1440 and 390, both modes, default palette: nav, settings,
  bookmarks, search, account, auth modal, command palette open, offline banner visible.

## Risks

- **The reader is the product.** A ground change that makes long reading worse is a
  failure even if every guard passes. V10 is a judgement check and should be done by a
  human reading actual Quran text for a few minutes, not by a diff.
- **Losing the warm reading surface** is a real regression for users who chose sepia
  deliberately. Do not let it disappear as a side effect of a palette remap — decide it.
- **Auth and account surfaces are easy to forget** and have their own form primitives
  (`lib/auth/components/`). They are covered by `auth-form-focus.test.ts`; run it.
- **The offline and notification surfaces** (`lib/offline/`, `components/notifications/`)
  render rarely and are usually missed in a visual sweep. V11 must force them visible.
