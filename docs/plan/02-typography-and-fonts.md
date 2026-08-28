# 02 — Typography and fonts

## Goal

Put Nunito on every Latin surface, self-hosted, and replace the nine ad-hoc heading sizes
with the single ramp the boards use — without touching the Quran/Arabic stack.

## Current state

`web/src/routes/layout.css:1–10` imports five self-hosted families:

```css
@import "@fontsource-variable/inter";
@import "@fontsource-variable/newsreader";
@import "@fontsource/noto-sans-arabic/400.css";
@import "@fontsource/noto-sans-arabic/600.css";
@import "@fontsource-variable/geist-mono";
@import "@fontsource/amiri";
```

Roles (`layout.css:13–20`): `--font-sans` = Inter (UI), `--font-display` = Newsreader
(editorial), `--font-arabic-ui` = Noto Sans Arabic, `--font-quran` resolves to the
reader's runtime choice with Amiri as the pre-hydration fallback, `--font-mono` = Geist
Mono.

The type scale (`layout.css:29–61`) has eleven role sizes — `display-xl` through `micro` —
plus eight retained legacy sizes (`--text-xs` … `--text-4xl`).

Arabic reader fonts are loaded at runtime through `FontFace` in
`web/src/lib/fonts/arabic-fonts.ts`, driven by `lib/config/reader-fonts.ts`. **None of
that changes.**

## Target state

### Families

| Token | From | To |
| --- | --- | --- |
| `--font-sans` | Inter Variable | **Nunito Variable** |
| `--font-display` | Newsreader | **removed** — the boards use one face |
| `--font-arabic-ui` | Noto Sans Arabic | unchanged |
| `--font-quran` | runtime choice, Amiri fallback | unchanged |
| `--font-mono` | Geist Mono Variable | unchanged |

```css
--font-sans: "Nunito Variable", "Nunito", ui-rounded, system-ui, -apple-system, "Segoe UI", sans-serif;
```

`ui-rounded` sits in the stack deliberately: it is the closest system fallback in
character, so a slow font load degrades to something with the same feel.

### The ramp

The boards use seven sizes. Map them onto the existing role tokens so consumers keep
their class names:

| Role token | Size | Line height | Weight | Used for |
| --- | --- | --- | --- | --- |
| `--text-display-xl` | 76px | 1.06 | 800 | Landing h1 |
| `--text-h1` | 40px | 1.10 | 800 | Band headings |
| `--text-h2` | 26px | 1.20 | 800 | Sub-headings |
| `--text-h3` | 20px | 1.25 | 800 | Card titles |
| `--text-body-xl` | 20px | 1.55 | 600 | Hero lead |
| `--text-body-l` | 17.5px | 1.60 | 600 | Body |
| `--text-body` | 15px | 1.50 | 600 | Small |
| `--text-caption` | 13.5px | 1.45 | 600 | Meta |
| `--text-micro` | 13px | 1.40 | 800 | Eyebrow, +0.1em tracking, uppercase |

Tracking is part of the ramp, not a per-use decision: −0.04em at 76px, −0.035em at 40px,
−0.03em at 26px, −0.025em at 20px, 0 below that.

Two ramp entries disappear (`--text-display-l`, `--text-body-s`). Keep them as aliases of
their nearest neighbour for one release rather than editing every call site at once.

### Body weight is 600, not 400

Nunito at 400 is too light for body text on a neutral ground at these sizes; the boards
use 600 throughout and 800 for headings. This is a real change to how the app reads and
should be checked at body-text scale early, not at the end.

### Arabic optical sizing

Amiri renders small next to Nunito at the same nominal size. The boards run Arabic at
**≈1.4×** the adjacent Latin size — 24px against 16.5px in surah rows, 36px in the closing
band. Encode it rather than leaving it to each call site:

```css
--font-size-arabic-ratio: 1.4;
```

…and a `.arabic` utility, or a `text-arabic-*` role set, that applies it. Do not scale the
Quran reading column this way — the reader has its own tuned sizing and is out of scope.

## Steps

1. **Confirm `@fontsource-variable/nunito` exists** at the version pinned by pnpm's
   catalogue policy, and add it to `web/package.json`. Nunito ships a variable weight axis
   covering 200–1000, which covers 600 and 800.
2. **Add the import** to `layout.css` and set `--font-sans`. Leave Inter imported for one
   commit so the fallback chain is testable, then remove it.
3. **Retire `--font-display`.** Grep `font-display` and `font-serif` consumers first —
   marketing headings and the Bismillah are the likely users. Replace with `--font-sans`
   at the ramp's heading weights.
4. **Rewrite the ramp** in the `@theme` block with the table above.
5. **Add the Arabic ratio** and the utility.
6. **Remove `@fontsource-variable/inter` and `@fontsource-variable/newsreader`** from both
   `layout.css` and `package.json` once no consumers remain.
7. **Check the offline pack.** `web/scripts/gen-offline-pack.ts` runs `postbuild` and
   enumerates what the service worker precaches. It does not currently name fonts
   explicitly, which means fonts are picked up by whatever glob or manifest it walks —
   confirm the new woff2 files land in the pack, and that the removed Inter/Newsreader
   files leave it. **This is the step most likely to be forgotten and only noticed
   offline.**

## Verification

### Machine

- `pnpm check`, `pnpm lint`, `pnpm test`.
- New guard `fonts.test.ts`:
  - `layout.css` contains **no** `fonts.googleapis.com` and no `@import url(http`;
  - `--font-sans` resolves to a Nunito family;
  - `@fontsource-variable/inter` and `newsreader` appear in neither `layout.css` nor
    `package.json`;
  - every `--text-*` role token defines size, line-height and weight.
- Grep gate for the whole repo: `rg "fonts\.(googleapis|gstatic)" web/src web/static` must
  be empty. The mockups contain these links; a copy-paste from `design/` would introduce
  one silently.

### Visual

Plan 07 checks **V3** (type specimen) and **V4** (offline fonts):

- **V3** renders the full ramp — every role, both weights, Latin and Arabic side by side —
  and is compared against the boards' rendered text at the same sizes.
- **V4** loads the app with the network throttled to offline after a first visit and
  confirms no font falls back to a system face. This is the check that catches a broken
  offline pack.

## Risks

- **A CDN font reference would break offline reading.** Not a style bug — a functional
  one. Guarded above.
- **Weight 600 body text increases rendered ink.** If it reads heavy at 15px, the fix is
  the ramp, not per-component overrides.
- **Removing `--font-display` is a wide grep.** Do it as its own commit so it can be
  reverted alone.
- **Nunito's Arabic coverage is irrelevant but its fallback order is not.** If a string
  mixes Latin and Arabic, the first family in the stack that has the glyph wins — make
  sure `.arabic` sets the family explicitly rather than relying on fallback.
