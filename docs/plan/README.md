# Design-system migration — COMPLETE

Status: **landed 2026-08-31.** This directory originally held the master plan plus nine
sub-plans (00–08) for migrating the web app from "Sacred Editorial"
(`docs/design-system.md` v1) to the pill/cobalt system. Every phase shipped; the
sub-plan files were removed and this single record now stands in their place. The full
per-phase specs and step-by-step rationale are preserved in git history — see the
commit ranges below (and `git log --oneline -- docs/plan/`).

## Where the truth lives now

- **Design truth:** `docs/design-system.md` (v2) — tokens, geometry, states, guards.
- **Migration history:** the commits below.

## What landed, by phase

| Phase | Content | Commits |
| --- | --- | --- |
| 0 | Visual harness: Puppeteer capture/contrast/states/matrix scripts, `/design/tokens` host, oklch contrast gate in `pnpm test` | `aee24786`, `22d86b6f` |
| 1 | Foundations: zero-chroma neutral ground ×8 palette blocks, mode-dependent hue set (light amber+lime / dark ember+forest), accent remap (sacred→cobalt, ink→neutral, sepia→magenta, sapphire→emerald), elevation zero | `868dbd5a`, `2a503239` |
| 2 | Typography: Nunito self-hosted, type ramp, `--font-display` retired, Inter/Newsreader dropped, offline pack verified | `1c873fc4` |
| 3 | Geometry: radius scale (pill / 8px holders / 10px blocks), 43× `rounded-full` audit, primitives + MetricCard + state matrix | `79a2271c` |
| 4 | Layout: `Band` full-bleed primitive + responsive ramp as utility ladder, RTL logical-property sweep + guard | `80b5c183`, `9f12b584`, `ab35de0a` |
| 5 | Marketing: landing rebuilt to the 8-band board, hero pill search (TanStack hotkeys), gapless hue metric strip, 5 sibling pages | `4fcf0b9a`, `dc0f835c`, `7c9204bc` |
| 6 | App + reader chrome: reader/status tokens, `accentHex` picker, ~20 chrome surfaces — reading column untouched | `a5bfb525`…`5cb70e96` (10) |
| 7 | Docs: `docs/design-system.md` rewritten to v2 code truth | `9d39360b`, `9f374f0e` |
| — | Loop fixes: twMerge type-role/text-colour collision (invisible CTA), CTA focus ring, contrast gate hardening | `3cb5d55e`, `44384693` |
| — | Colour extension (follow-up): hue grammar brought to app surfaces — dashboard metric strip, juz hue tiles, search/settings active fills, reader chrome bands | `53ec73a4`, `1dccc0ec`, `dba9633a`, `93a16cbb`, `6382d9db` |
| — | Reader green-token fixes (follow-up): ayah marker + pop/accent-line alias chain moved to primary family; prev/next pill fills | `2f24288a`, `c0037809` |

All gates green throughout: `pnpm check`, `pnpm lint`, `pnpm test` (final: 151 files /
1,925 tests). Visual acceptance verified per palette × mode by screenshot evidence.

## Open items for the owner

Deferred during the migration — none blocking, all judgement calls:

1. **Sepia warm-reader** — plan 06 option (a) was implemented (warm reader both modes);
   ratify or queue option (b) as an orthogonal warmth setting.
2. **`Section.svelte`** — pre-existing dead code, zero consumers; delete (and then
   judge `Container`).
3. **CI** — no workflow runs `pnpm test`; the compiled-CSS ladder guard is local-only.
4. **Band heights** drift −4…−12px vs the boards at 1440px; hero pill 820×76 vs board
   860×78.
5. **Accent utilities** — `bg-accent`/`text-accent` retirement (`--color-accent` =
   primary vs `--accent` = hue-2 dual meaning documented in design-system §8).
6. **Harness hardening** — V8 comparator script, `states.ts` transition race, missing
   V3/V5 host pages; `reference.ts` ignores `--out`.
7. **"Powered by Gemini" watermark** — only accidental colour left on app pages;
   audit recommended KEEP (attribution safety) — removal is an aesthetic call.
8. **Small contrast/aesthetic nits** — juz dark range chip faint; light hue-2 tile
   numerals 4.09:1 (large-text pass only); tooltip arrow tip flattened by the ui/
   literal ban; `lg` pill padding extrapolates above board curve.
9. **Dead tokens** — `--pop`/`--pop-soft`/`--accent-line` now consumer-less; delete
   with their tailwind aliases.
10. **SyncIndicator** — restyle verified token-level only (auth-gated in dev); take a
    live screenshot when convenient.
