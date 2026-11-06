# Build plan

The master index. It holds the build order; the detail for each item lives in its sub-plan. One agent implements it top to bottom, one item at a time.

## Sub-plans

| Plan                                                     | Covers                                                                        |
| -------------------------------------------------------- | ----------------------------------------------------------------------------- |
| [rules.md](plans/rules.md)                               | Standing rules, closed decisions, what already shipped. Applies to every item |
| [sync.md](plans/sync.md)                                 | Multi-device conflict protocol                                                |
| [devices.md](plans/devices.md)                           | Device registry, fingerprint, continuity, native client contract              |
| [account-ux.md](plans/account-ux.md)                     | Account page                                                                  |
| [abuse-0-foundations.md](plans/abuse-0-foundations.md)   | Truth, measurement, ingress                                                   |
| [abuse-1-reader-login.md](plans/abuse-1-reader-login.md) | Reader path and login                                                         |
| [abuse-2-enforcement.md](plans/abuse-2-enforcement.md)   | Ban contract and enforcement                                                  |
| [product.md](plans/product.md)                           | Product features and copy                                                     |
| [native-shared.md](plans/native-shared.md)               | Shared app foundations: repo, feature matrix, Quran data, deep links, privacy |
| [mobile.md](plans/mobile.md)                             | React Native app                                                              |
| [desktop.md](plans/desktop.md)                           | GPUI desktop app                                                              |
| [edge-cases.md](plans/edge-cases.md)                     | Every known scenario and its required behaviour                               |
| [owner.md](plans/owner.md)                               | Human-only steps                                                              |

Research that items cite stays in `research/abuse-prevention/`.

## Scope

Web items come first, then the apps. If app work must start sooner, move the Native, Mobile and Desktop sections up; their blockers still apply. `contracts/` holds what all clients must obey and outlives the items that create it.

## How to work

1. Take the first item in **Build order** below whose `Blocked by` items are all gone from this index. Read it in full in its sub-plan. Skip blocked items and come back to them.
2. Build it and its tests. Follow `AGENTS.MD`, [the rules](plans/rules.md), and the area docs it names.
3. Get gates green: `bun run check`, `bun run lint`, `bun run test` from `web/`; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` for Rust changes.
4. Delete the finished item from its sub-plan and its row from this index in the same commit. One commit per item, on the current branch. Never push.
5. An item is done only when every [edge-case ledger](plans/edge-cases.md) row naming it has a test, or is marked manual and listed in O32. Copy those rows into the contract file the row names. Delete a row once all its items are finished.
6. Repeat until only blocked items remain.

`Blocked by` stops an item. `Owner` names a human step for real-world rollout: build anyway, keep the feature behind config or off until that step is done, and never wait for it. Owner steps live in [owner.md](plans/owner.md); the owner deletes each one when done. A `Blocked by` naming an `O<n>` clears the same way.

If reality differs from an item, trust the code, adjust, and note it in the commit message. New work found while building goes in as a new item at the right position, in its sub-plan and this index. Stop and ask only if an item would break a rule in `AGENTS.MD`.

## Build order

### Abuse phase 0

| Item                                                                                                             | Title                                                                         | Blocked by |
| ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------- |
| [A4b](plans/abuse-0-foundations.md#a4b-contact-addresses) | Contact addresses | O3 |

### Devices

| Item                                                                   | Title                                        | Blocked by |
| ---------------------------------------------------------------------- | -------------------------------------------- | ---------- |
| [D3](plans/devices.md#d3-device-fingerprint-and-continuity)            | Device fingerprint and continuity            | D2         |
| [D4](plans/devices.md#d4-devices-on-the-account-page)                  | Devices on the account page                  | D2, U1     |
| [D5](plans/devices.md#d5-handoff-by-device)                            | Handoff by device                            | D3, S9     |
| [D6](plans/devices.md#d6-native-client-contract-and-session-transport) | Native client contract and session transport | D3, D5     |

### Abuse phase 1

| Item                                                                                                        | Title                                                                 | Blocked by |
| ----------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ---------- |
| [A8](plans/abuse-1-reader-login.md#a8-login-limiter-split-failures-only-no-lock)                            | Login limiter split: failures only, no lock                           | A5         |
| [A9](plans/abuse-1-reader-login.md#a9-registration-admission-password-and-oauth-doors)                      | Registration admission, password and OAuth doors                      | A8         |
| [A10](plans/abuse-1-reader-login.md#a10-verification-identity-canonicalization)                             | Verification identity canonicalization                                | A9         |
| [A11](plans/abuse-1-reader-login.md#a11-password-work-queue-and-reserved-lanes)                             | Password-work queue and reserved lanes                                | A8         |
| [A12](plans/abuse-1-reader-login.md#a12-recovery-masking-reset-grants-email-change)                         | Recovery masking, reset grants, email change                          | A8         |
| [A13](plans/abuse-1-reader-login.md#a13-challenge-maps-no-eviction-per-session-quota-session-payload-state) | Challenge maps: no eviction, per-session quota, session payload state | A12        |
| [A14](plans/abuse-1-reader-login.md#a14-reader-path-cookie-and-cache-predicate)                             | Reader-path cookie and cache predicate                                | -          |
| [A15](plans/abuse-1-reader-login.md#a15-web-origin-limiter-internal-bucket-split-prod-boot-error)           | Web-origin limiter, internal bucket split, prod boot error            | A1, A5     |
| [A16](plans/abuse-1-reader-login.md#a16-offline-pack-budget-and-cache-semantics)                            | Offline pack budget and cache semantics                               | A15        |
| [A17](plans/abuse-1-reader-login.md#a17-canonical-redirects-no-store-errors-per-artifact-verified-set)      | Canonical redirects, no-store errors, per-artifact verified set       | -          |
| [A18](plans/abuse-1-reader-login.md#a18-bulk-data-path-and-observability)                                   | Bulk-data path and observability                                      | A5         |

### Abuse phase 2

| Item                                                                                                    | Title                                                             | Blocked by   |
| ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------ |
| [A19](plans/abuse-2-enforcement.md#a19-ban-contract-core-migration-admin-api-session-kill)              | Ban contract core: migration, admin API, session kill             | -            |
| [A20](plans/abuse-2-enforcement.md#a20-enforcement-correctness-and-banned-ux)                           | Enforcement correctness and banned UX                             | A19          |
| [A21](plans/abuse-2-enforcement.md#a21-notice-delivery-suppression-result-appeal-clock)                 | Notice delivery, suppression result, appeal clock                 | A19          |
| [A35](plans/abuse-2-enforcement.md#a35-appeal-intake)                                                   | Appeal intake                                                     | A21          |
| [A22](plans/abuse-2-enforcement.md#a22-shared-device-precedence-and-ban-decision-fields)                | Shared-device precedence and ban-decision fields                  | A19          |
| [A23](plans/abuse-2-enforcement.md#a23-audit-writer-and-call-sites)                                     | Audit writer and call sites                                       | A19          |
| [A24](plans/abuse-2-enforcement.md#a24-incident-intake-reports)                                         | Incident intake: reports                                          | A23          |
| [A25](plans/abuse-2-enforcement.md#a25-case-evidence-legal-holds-incident-capture-delete-guard)         | Case evidence, legal holds, incident capture, delete guard        | A23          |
| [A26](plans/abuse-2-enforcement.md#a26-alerting-runbook-drill)                                          | Alerting, runbook, drill                                          | A24, A25     |
| [A27](plans/abuse-2-enforcement.md#a27-moderation-notice-and-comment-hardening)                         | Moderation notice and comment hardening                           | A21, A25     |
| [A28](plans/abuse-2-enforcement.md#a28-admin-safety)                                                    | Admin safety                                                      | A23          |
| [A29](plans/abuse-2-enforcement.md#a29-mail-budget-bounce-guard-suppression-governance-unsubscribe)     | Mail budget, bounce guard, suppression governance, unsubscribe    | A21, A23     |
| [A36](plans/abuse-2-enforcement.md#a36-device-token-hygiene)                                            | Device token hygiene                                              | A23          |
| [A30](plans/abuse-2-enforcement.md#a30-media-takedown-variants-tombstones-sweep)                        | Media takedown: variants, tombstones, sweep                       | A23, A28     |
| [A37](plans/abuse-2-enforcement.md#a37-image-moderation-and-csam-path)                                  | Image moderation and CSAM path                                    | A26, A30     |
| [A31](plans/abuse-2-enforcement.md#a31-media-upload-hardening-and-gate)                                 | Media upload hardening and gate                                   | A30          |
| [A32](plans/abuse-2-enforcement.md#a32-retention-sweeper-with-ship-gates)                               | Retention sweeper with ship gates                                 | A5, A19, A25 |
| [A33](plans/abuse-2-enforcement.md#a33-erasure-receipts-physical-deletion-backups)                      | Erasure receipts, physical deletion, backups                      | A32          |
| [A38](plans/abuse-2-enforcement.md#a38-erasure-and-data-request-procedure)                              | Erasure and data-request procedure                                | A26, A33     |
| [A34](plans/abuse-2-enforcement.md#a34-policy-text-sentence-ledger-minors-code-side-of-legal-artifacts) | Policy text sentence ledger, minors, code side of legal artifacts | A4, A26, A32 |

### Product

| Item                                                               | Title                                  | Blocked by |
| ------------------------------------------------------------------ | -------------------------------------- | ---------- |
| [F1](plans/product.md#f1-remove-dead-tafsir-placeholder)           | Remove dead tafsir placeholder         | -          |
| [F2](plans/product.md#f2-settings-and-widget-layout-sync)          | Settings and widget-layout sync        | S7         |
| [F3](plans/product.md#f3-notes-library)                            | Notes library                          | -          |
| [F4](plans/product.md#f4-thread-history-clear-all)                 | Thread history clear-all               | S7         |
| [F5](plans/product.md#f5-explicit-reading-bookmark)                | Explicit reading bookmark              | S7         |
| [F6](plans/product.md#f6-personal-data-export-and-import)          | Personal data export and import        | -          |
| [F7](plans/product.md#f7-self-service-account-deletion)            | Self-service account deletion          | F6, A38    |
| [F8](plans/product.md#f8-reading-goals-and-streaks)                | Reading goals and streaks              | S7         |
| [F9](plans/product.md#f9-goal-reminders)                           | Goal reminders                         | F8         |
| [F10](plans/product.md#f10-pin-and-compare-verses)                 | Pin and compare verses                 | -          |
| [F11](plans/product.md#f11-parallel-translation-layout)            | Parallel translation layout            | -          |
| [F12](plans/product.md#f12-deeper-navigation)                      | Deeper navigation                      | -          |
| [F13](plans/product.md#f13-share-dialog-and-image-card)            | Share dialog and image card            | -          |
| [F14](plans/product.md#f14-bookmark-bulk-actions)                  | Bookmark bulk actions                  | S7         |
| [F15](plans/product.md#f15-voice-search)                           | Voice search                           | -          |
| [F16](plans/product.md#f16-public-daily-ayah-route)                | Public daily ayah route                | -          |
| [F17](plans/product.md#f17-translation-correction-report)          | Translation correction report          | A24        |
| [F18](plans/product.md#f18-rtl-polish-and-localized-text-variants) | RTL polish and localized text variants | -          |
| [F19](plans/product.md#f19-recitation-audio-and-player)            | Recitation audio and player            | O22        |
| [F20](plans/product.md#f20-memorization-repeat-engine)             | Memorization repeat engine             | F19        |
| [F21](plans/product.md#f21-word-by-word)                           | Word-by-word                           | O23        |
| [F22](plans/product.md#f22-more-tafsir-sources)                    | More tafsir sources                    | O24        |

### Copy

| Item                                      | Title           | Blocked by |
| ----------------------------------------- | --------------- | ---------- |
| [C1](plans/product.md#c1-copy-truth-pass) | Copy truth pass | -          |

### Native foundations

| Item                                                                            | Title                                            | Blocked by |
| ------------------------------------------------------------------------------- | ------------------------------------------------ | ---------- |
| [N1](plans/native-shared.md#n1-repo-layout-and-shared-client-core)              | Repo layout and shared client core               | S9         |
| [N2](plans/native-shared.md#n2-feature-scope-per-client)                        | Feature scope per client                         | -          |
| [N3](plans/native-shared.md#n3-quran-data-on-native)                            | Quran data on native                             | N1         |
| [N4](plans/native-shared.md#n4-deep-links-and-association-files)                | Deep links and association files                 | N2         |
| [N5](plans/native-shared.md#n5-analytics-crash-reporting-and-privacy-on-native) | Analytics, crash reporting and privacy on native | N2         |

### Mobile app

| Item                                                             | Title                                       | Blocked by |
| ---------------------------------------------------------------- | ------------------------------------------- | ---------- |
| [M1](plans/mobile.md#m1-app-scaffold)                            | App scaffold                                | N1, D6     |
| [M3](plans/mobile.md#m3-session-and-device-identity)             | Session and device identity                 | M1, N4     |
| [M4](plans/mobile.md#m4-local-store-and-sync)                    | Local store and sync                        | M3, N3     |
| [M5](plans/mobile.md#m5-reader)                                  | Reader                                      | M4         |
| [M6](plans/mobile.md#m6-yours-bookmarks-search-settings-devices) | Yours, bookmarks, search, settings, devices | M5         |
| [M7](plans/mobile.md#m7-push-and-reminders)                      | Push and reminders                          | M6         |
| [M8](plans/mobile.md#m8-localization-and-text-size)              | Localization and text size                  | M6         |
| [M9](plans/mobile.md#m9-release)                                 | Release                                     | M7, M8, N5 |

### Desktop app

| Item                                                              | Title                                       | Blocked by |
| ----------------------------------------------------------------- | ------------------------------------------- | ---------- |
| [G1](plans/desktop.md#g1-app-scaffold)                            | App scaffold                                | N1, D6     |
| [G2](plans/desktop.md#g2-production-arabic-text-element)          | Production Arabic text element              | G1         |
| [G3](plans/desktop.md#g3-session-and-device-identity)             | Session and device identity                 | G1, N4     |
| [G4](plans/desktop.md#g4-local-store-and-sync)                    | Local store and sync                        | G3, N3     |
| [G5](plans/desktop.md#g5-reader)                                  | Reader                                      | G2, G4     |
| [G6](plans/desktop.md#g6-yours-bookmarks-search-settings-devices) | Yours, bookmarks, search, settings, devices | G5         |
| [G7](plans/desktop.md#g7-localization-and-keyboard)               | Localization and keyboard                   | G6         |
| [G8](plans/desktop.md#g8-packaging-and-updates)                   | Packaging and updates                       | G7, N5     |
