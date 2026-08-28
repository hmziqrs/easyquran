# 08 — Sequencing and rollback

## Goal

An order that keeps the app shippable at every commit, and a way back from each phase.

## Ordering principle

**Tokens before components before pages.** Each layer is consumed by the one after it, so
a change to the ground propagates for free — but only if the ground moves first. Doing it
in the other order means restyling every component twice.

The second principle: **build the check before the change it checks.** V1 (token sweep)
and the contrast gate come before the token swap, not after.

## Phases

| Phase | Plan | Lands | App state after |
| --- | --- | --- | --- |
| **0** | 07 | Visual harness, V1, contrast gate | Unchanged, now measurable |
| **1** | 01 | Hue tokens added (inert) | Unchanged |
| **2** | 01 | Ground → neutral, one palette per commit | **Visibly different everywhere** |
| **3** | 01 | Accents, `--accent` repoint, elevation zeroed | New colour, old type and geometry |
| **4** | 02 | Nunito, ramp, Inter/Newsreader out | New colour and type |
| **5** | 03 | Radii, primitives, states | New colour, type, geometry |
| **6** | 04 | `Band`, `Section` over `Band`, RTL sweep | Same layout, new mechanism |
| **7** | 05 | Landing rebuilt, marketing swept | Landing matches the boards |
| **8** | 06 | App chrome, reader tokens, palette picker | Whole app on the new system |
| **9** | — | `docs/design-system.md` updated | One design truth in the repo |

### Phase 2 is the sharp edge

Everything before it is invisible; everything after it is incremental. The moment the
ground goes neutral, every one of the 92 components is sitting on a surface it was not
designed for. Mitigations:

- One palette per commit, V1 + V6 between each.
- Land it early in a working session, not at the end of one.
- Expect to fix contrast fallout in the same phase — components that relied on a tinted
  ground for separation will need a border or a surface step.

### Phase 9 is not optional

`docs/design-system.md` (2,526 lines) describes Sacred Editorial in detail and is cited by
`palette-contract.test.ts` and `primitives.test.ts` in their doc comments. Leaving it
stale means the repo documents a design it no longer has. Either update it or replace it
with a new document and leave a pointer — but do not leave two competing truths.

## Commit discipline

- One phase per branch; one logical change per commit.
- A guard update lands in the **same commit** as the change it guards. Never split.
- Commit messages name the plan: `feat(web): neutral ground for sacred palette (plan 01)`.
- Per `AGENTS.MD`: commit to the current branch, and run all three gates before each commit
  — `pnpm check`, `pnpm lint`, `pnpm test`.

## Rollback

| Phase | How to back out | Cost |
| --- | --- | --- |
| 0 | Delete `scripts/visual/`, drop the contrast test | None — nothing depends on it |
| 1 | Revert the token additions | None — they are unused |
| 2–3 | Revert the palette blocks | Clean: `layout.css` only, no component depends on the values |
| 4 | Revert `layout.css` font lines + restore `package.json` deps | Clean if `--font-display` removal is its own commit |
| 5 | Revert per primitive | Clean per file; the guard reverts with it |
| 6 | `Section` still exists and is output-identical, so reverting `Band` consumers is safe | Low |
| 7 | Revert the landing page; i18n keys can stay (unused keys do not fail parity) | Low |
| 8 | Revert per surface | Low |

**The expensive irreversible decision is D1** (palette id semantics). If the ids were
renamed instead of remapped, rollback means a data migration. This is the reason plan 00
recommends remapping.

## Risk register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| A CDN font reference ships | Medium — the mockups contain them | **High** — breaks offline reading | Grep gate, plan 02 |
| Contrast regression on a non-default palette | High | High | Contrast gate, plan 07 |
| Reader becomes worse to read | Medium | **High** — it is the product | V10 human check, plan 06 |
| `rounded-full` audit mis-classifies | High | Low, visible | Explicit split list, plan 03 |
| i18n parity break blocks all three gates | Medium | Medium | Add `ar.json` keys in the same commit |
| Warm reading surface silently lost | Medium | Medium | Explicit decision, plan 06 |
| Hero search regresses the primary action | Low | High | Test submit path before styling, plan 05 |
| `primitives.test.ts` churn normalises guard edits | Medium | Medium | Guard updates ride with their change |

## What "done" looks like

1. Three gates green.
2. Contrast gate green across all eight palette blocks.
3. V1–V11 run and reviewed; V10 done by a human on real text.
4. No `fonts.googleapis.com`, no hex literal in a component, no physical-direction class
   on a migrated surface.
5. `/` matches the boards on the V8 structural comparison in both modes and all four
   palettes.
6. `docs/design-system.md` reflects what the app now is.

## Sizing

Rough, assuming one person and no parallelism:

| Phase | Effort |
| --- | --- |
| 0 | 1–2 days — the harness is real work and pays for itself |
| 1–3 | 1–2 days, mostly verification between commits |
| 4 | half a day, plus offline-pack verification |
| 5 | 2–3 days — the `rounded-full` audit and the state matrix dominate |
| 6 | half a day |
| 7 | 2–3 days including i18n and the search wiring |
| 8 | 2–3 days across ~20 surfaces |
| 9 | half a day |

Phases 5, 7 and 8 carry the bulk. Phase 0 is the one most likely to be skipped under time
pressure and the one whose absence costs the most later.
