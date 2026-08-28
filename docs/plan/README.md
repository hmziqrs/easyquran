# Design-system migration — master plan

Migrating the web app from **Sacred Editorial** (`docs/design-system.md`, v1.0) to the
direction signed off on 2026-08-29.

## Source of truth

Two files in the untracked `design/` directory are the visual contract. Everything in
this plan is derived from them; when a sub-plan and a mockup disagree, the mockup wins.

| File | Role |
| --- | --- |
| `design/PillLightCobalt.dc.html` | Light ground, cobalt accent — the canonical light board |
| `design/white-on-colour-deep-cobalt.html` | Dark ground, deep cobalt, white on every coloured surface — the canonical dark board |

`design/PillLightCobalt.dc.html` is a Design Component file: it carries a `<x-dc>` wrapper
and a `<helmet>` block. Strip those to read it as plain HTML (the dark file is already
plain). Neither is a build input — they are references only.

The canvas the boards came from: <https://claude.ai/code/artifact/b5ddc106-b844-46b7-91fe-0999778b6df0>

## What actually changes

| Layer | From | To |
| --- | --- | --- |
| Ground | Four tinted palettes (forest / graphite / parchment / slate) | One **zero-chroma neutral** ground, light and dark |
| Colour | Green primary + antique gold editorial accent | **Vibrant accent + a four-hue card set**, all colour concentrated in fills |
| Type | Inter (UI) + Newsreader (editorial) | **Nunito** everywhere, Amiri unchanged for Quran/Arabic |
| Geometry | 10–18px radii throughout | **Pill controls, 8px square icon holders, 10px blocks** |
| Section shape | Contained cards inside a max-width column | **Full-bleed bands**, content stopping at a gutter |
| Dark mode | Warm charcoal | Neutral charcoal, coloured surfaces carry **white** text |

## Sub-plans

Read them in order. Each is independently landable and independently verifiable.

| # | Plan | Lands |
| --- | --- | --- |
| 00 | [Scope and decisions](00-scope-and-decisions.md) | Decisions that must be settled before code moves |
| 01 | [Foundations — tokens](01-foundations-tokens.md) | `layout.css` token contract, palette blocks, new hue tokens |
| 02 | [Typography and fonts](02-typography-and-fonts.md) | Nunito self-hosted, type ramp, Arabic optical sizing |
| 03 | [Geometry and primitives](03-geometry-and-primitives.md) | Radius scale, Button/IconButton/Input/Tabs/Chip |
| 04 | [Layout — full-bleed bands](04-layout-full-bleed.md) | `Section`/`Container` → band model |
| 05 | [Marketing and landing](05-marketing-landing.md) | The landing page itself, plus i18n keys |
| 06 | [App and reader surfaces](06-app-and-reader-surfaces.md) | Chrome, nav, reader, settings |
| 07 | [Visual verification](07-visual-verification.md) | The screenshot harness and per-phase acceptance |
| 08 | [Sequencing and rollback](08-sequencing-and-rollback.md) | Commit order, risk, how to back out |

## Non-negotiables carried from `AGENTS.MD`

These constrain every phase and are repeated in the sub-plans where they bite:

- **Self-hosted fonts only.** Never a CDN — the service worker and offline pack depend on
  it. The mockups link Google Fonts; the implementation must not. See plan 02.
- **All three gates stay green:** `pnpm check` (warning-free, `--fail-on-warnings`),
  `pnpm lint` (`--deny-warnings`), `pnpm test`.
- **No nested ternaries** anywhere, including `$derived` and test files.
- **No SHA-256 over Quran data** in any automated path — unrelated to this work, but the
  guards in `lib/quran/__tests__/catalogue-sha-guard.test.ts` must keep passing.
- **Reader navigation hrefs** must keep using the `surah*For(ctx, …)` family; the
  `nav-guard.test.ts` guard fails the build otherwise. Restyling must not touch hrefs.
- Lint rules live in `web/vite.config.ts` → `lint.rules`. Never add `.oxlintrc.json`.

## Guards this migration must keep passing or deliberately update

| Guard | Location | Effect of this migration |
| --- | --- | --- |
| Palette contract | `web/src/routes/__tests__/palette-contract.test.ts` | **Updated** — plan 01 |
| Primitives §61 | `web/src/lib/components/ui/__tests__/primitives.test.ts` | **Extended** — plan 03 |
| Chip | `web/src/lib/components/chip/__tests__/chip.test.ts` | **Extended** — plan 03 |
| Theme derive | `web/src/lib/theme/__tests__/derive.test.ts` | **Updated** — plan 01 |
| Prefs | `web/src/lib/stores/__tests__/prefs.test.ts` | Unchanged if palette ids are preserved — plan 00 |
| Reader fonts | `web/src/lib/config/__tests__/reader-fonts.test.ts` | Unchanged — Arabic stack is out of scope |
| Marketing surface | `web/src/lib/i18n/__tests__/marketing-surface-guard.test.ts` | **Extended** — plan 05 |
| Nav hrefs | `web/src/routes/(application)/app/__tests__/nav-guard.test.ts` | Must stay green untouched |

## How to use this plan

Each sub-plan has the same shape:

1. **Goal** — one sentence.
2. **Current state** — what is there now, with file paths and line references.
3. **Target state** — what it becomes, with concrete values.
4. **Steps** — ordered, each small enough to land alone.
5. **Verification** — machine checks plus the visual checks from plan 07.
6. **Risks** — what breaks and what to do about it.

Nothing in this plan changes Quran data, the reader's data path, sync, or auth.
