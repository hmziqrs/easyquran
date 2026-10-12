# next-features.md — post-merge follow-ups + verification ledger

`docs/next-features.md` is the design record for the feature batch that shipped as PR #31 (merge
`223a2f0e`, branch `next-features`, landed 2026-10-12). Decision (2026-10-12): **the doc itself
stays untouched** — it remains a stable pre-implementation baseline, so verification passes compare
against a known state with no edits mixed in. Every pending correction, verified fact, and open
item lives HERE instead; apply to `next-features.md` only after a proper verification round, in one
deliberate pass.

Sequencing: that pass is phase **V** in [`master-plan.md`](../master-plan.md) — QA FLOW-06, apply
this ledger in one pass, then retire `next-features.md`, `locale-suggest.md`, and this ledger
together.

A fully corrected candidate text for `next-features.md` exists in commit `855481a1` ("docs: land
the parked docs-followups salvage…") — diff that file against current master before landing.

## Verified facts (checked against code 2026-10-12)

| next-features.md says (stale)                                                                                         | Reality on master                                                                                                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "Migrations end at `m000011`; next is `m000012`" (§0)                                                                 | End at `m000014` (`user_analytics`, this batch); next free **`m000015`** (`migration/src/lib.rs`)                                                                                                        |
| "swap the discard for retain-and-review before the next web deploy" (§0, §9.7, §11.14, §12 step 0, §14 item 4, EC-48) | **Done** — `applyLegacyUnowned` + `unownedPending` in `web/src/lib/bookmarks/store.svelte.ts`; the `#discardUnownedQueue()` call is gone; EC-48 tests in `bookmarks/__tests__/account-isolation.test.ts` |
| "`#migrateLegacy`" (`bookmarks/store.svelte.ts:450`) (§9.4, EC-10, EC-11)                                             | Symbol retired; non-silent replacement shipped as `web/src/lib/components/auth/merge-review.svelte.ts`                                                                                                   |
| Feature 5 widget registry `web/src/lib/home/widgets.ts`                                                               | Shipped as `web/src/lib/home/layout.svelte.ts` (`HOME_WIDGET_SPECS`)                                                                                                                                     |
| Feature 6 migration "next free number after `m000013`"                                                                | Shipped as `m000014` + `modules/user_analytics_v1` (the exact §9.6 schema)                                                                                                                               |
| Feature 10 source "`locale-suggest.md` (draft, not implemented)"                                                      | Shipped: `web/src/lib/components/i18n/LocaleSuggest.svelte` + `web/src/lib/i18n/locale-suggest.svelte.ts`, wired in the root layout                                                                      |
| "2026-09-30 UX audit" (header, §13 table)                                                                             | Audit dated **2026-09-29** (commits `b09d3b05`, `525f62b5`)                                                                                                                                              |
| R2-08 = `fbf5edcd`/`d1c47299`, "committed not deployed" (§0, §9.7)                                                    | Commits correct and on master; deployment state not verifiable from the repo                                                                                                                             |
| `bookmark_folder/slice.rs` (Feature 2.2)                                                                              | Real path: `rust/backend/api/src/db/sea_models/bookmark_folder/{actions,model,slice}.rs`                                                                                                                 |

## Dead references inside next-features.md (line numbers at `223a2f0e`)

Paths removed by the 2026-10-12 docs cleanup, still cited by the doc:

- `research/ux-audit` (folder deleted; live replacement = `scripts/ux-audit/` harness +
  `documentation/ux-audit-playbook.md`) — lines 9, 538, 556, 587, 1161, 1300, 1309, 1327, 1329, 1331,
  1337, 1351
- `plan/09-locale-suggest.md` → live: `docs/locale-suggest.md` — lines 10, 613, 1310, 1339, 1349
- `docs/research/translation-empty-verses.md` → live: `documentation/quran-system.md` "Sources and
  metadata" (lists the same 4 blank verses) — lines 100, 1317
- `navigation-audit.md` / `url-scheme-a.md` → live: `documentation/history.md` (navigation audit + URL
  scheme A, both implemented; rubʿ routes retired) — lines 446, 1318, 1345
- `reader-virtualization.md` → live: `documentation/quran-system.md` "Reader rendering" — lines 1177, 1320
- `security-audit-2026-10-11.md` / `security-audit-rounds-2026-10-11.md`, `plan/README.md`,
  `plans/remove-primary-translation.md`, `design-enhancements/` — §13/§14 area, lines 1315–1348

## Open verification / QA items

1. FLOW-06 (§13 "Looks shipped — verify"): ⌘K go-to-verse — confirm in QA, then tick off.
2. C03 recitation copy (pre-release): Terms + FAQ still promise recitation audio
   (`web/messages/en.json` `legal_terms_service_body`, `legal_terms_third_party_body`,
   `faq_offline_answer`) and no audio player exists. The tafsir half of C03 is resolved — see
   `remaining/copy-corrections.md` C03.
3. `documentation/ux-audit-playbook.md:94` points future audit output at the deleted
   `docs/research/ux-audit/` folder — decide the new output home.
4. `remaining/feature-gap-catalogue.md` R11 note: update when §11.5 (range permalink grammar) is
   decided.
5. §11 open decisions not yet settled in the doc: widget-layout sync (11.3), GA consent default +
   granularity (11.4).

## Parked-branch history

Branch `docs-followups` (commits `f597f186`, `60c22447`; deleted 2026-10-12 after salvage) held
the pre-merge parked edits to `next-features.md`; its content is preserved in the `855481a1`
candidate text above. The copy-corrections half of that branch (dead `content.ts` links + the C03
tafsir fact refresh) landed directly in `docs/remaining/copy-corrections.md` — that file is a
living ledger, not part of the frozen baseline.
