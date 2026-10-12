# Documentation — permanent, human-owned records

This folder is the long-term reference tier. Everything here describes **shipped behaviour,
settled contracts, finished work, and durable methodology**.

**This folder is untouchable by agents.** Files here are created and edited manually by the owner
only — agents may read and cite them freely but never create, edit, move, or delete anything under
`documentation/`. A pre-commit hook refuses staged changes here unless the owner runs with
`DOCS_OVERRIDE=1`. Hot implementation plans live in [`docs/`](../docs/README.md) instead: they are
built, shipped, then retired — never kept as long-term resources. When a hot plan is fully
implemented and verified, the owner may distil a permanent record of it into this folder by hand.

## Contracts (shipped behaviour)

| Doc                                        | Owns                                                                                    |
| ------------------------------------------ | --------------------------------------------------------------------------------------- |
| [`quran-system.md`](quran-system.md)       | Quran data, API, delivery, caching, UI i18n, auth isolation, ingress — the top contract |
| [`search-system.md`](search-system.md)     | Client search, ranking, palette and search page                                         |
| [`settings-system.md`](settings-system.md) | Settings, storage administration, reader presentation                                   |
| [`design-system.md`](design-system.md)     | Tokens, geometry, states and design guards (v2 pill/cobalt)                             |
| [`qurancom-mirror.md`](qurancom-mirror.md) | quran.com reference mirror and licence register (test-guarded)                          |

## Records (overviews of finished work)

| Doc                        | Covers                                                                                        |
| -------------------------- | --------------------------------------------------------------------------------------------- |
| [`history.md`](history.md) | Design-system migration, primary-translation removal, navigation + URL scheme A, design study |
| [`indopak.md`](indopak.md) | IndoPak font v1–v4, verification, open editorial items                                        |

## Research and methodology

| Doc                                                            | Owns                                                     |
| -------------------------------------------------------------- | -------------------------------------------------------- |
| [`mushaf-rendering-research.md`](mushaf-rendering-research.md) | Research for a print-exact mushaf mode (not implemented) |
| [`ux-audit-playbook.md`](ux-audit-playbook.md)                 | How to run a UX audit; tooling in `scripts/ux-audit/`    |

[`my-plan-raw.md`](my-plan-raw.md) is the owner's own notes — agents never edit it; where shipped
architecture differs, `quran-system.md` Part 5 records why.
