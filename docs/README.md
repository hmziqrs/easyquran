# Docs

Code owns implementation detail; these documents record contracts, plans and decisions that must
survive refactors. Repo-wide hard rules live in [`AGENTS.MD`](../AGENTS.MD).

## Contracts (shipped behaviour)

| Doc                                        | Owns                                                                                    |
| ------------------------------------------ | --------------------------------------------------------------------------------------- |
| [`quran-system.md`](quran-system.md)       | Quran data, API, delivery, caching, UI i18n, auth isolation, ingress — the top contract |
| [`search-system.md`](search-system.md)     | Client search, ranking, palette and search page                                         |
| [`settings-system.md`](settings-system.md) | Settings, storage administration, reader presentation                                   |
| [`design-system.md`](design-system.md)     | Tokens, geometry, states and design guards (v2 pill/cobalt)                             |
| [`qurancom-mirror.md`](qurancom-mirror.md) | quran.com reference mirror and licence register (test-guarded)                          |

## Plans and backlog

| Doc                                                                              | Status                                                                                                                      |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| [`next-features.md`](next-features.md)                                           | Next feature batch + sync design; wins over older plans on conflict                                                         |
| [`abuse-prevention.md`](abuse-prevention.md)                                     | Draft abuse-prevention and enforcement plan; evidence in `research/abuse-prevention/`                                       |
| [`locale-suggest.md`](locale-suggest.md)                                         | Draft spec for next-features Feature 10                                                                                     |
| [`remaining/feature-gap-catalogue.md`](remaining/feature-gap-catalogue.md)       | Parity register against quran.com                                                                                           |
| [`remaining/copy-corrections.md`](remaining/copy-corrections.md)                 | Pre-release copy ledger                                                                                                     |
| [`research/ux-audit/`](research/ux-audit/README.md)                              | UX audit write-ups; status of all 200 findings in its [release checklist](research/ux-audit/27-release-polish-checklist.md) |
| [`research/mushaf-rendering-research.md`](research/mushaf-rendering-research.md) | Research for a print-exact mushaf mode (not implemented)                                                                    |

## Records (overviews of finished work)

| Doc                        | Covers                                                                                        |
| -------------------------- | --------------------------------------------------------------------------------------------- |
| [`history.md`](history.md) | Design-system migration, primary-translation removal, navigation + URL scheme A, design study |
| [`indopak.md`](indopak.md) | IndoPak font v1–v4, verification, open editorial items                                        |

[`my-plan-raw.md`](my-plan-raw.md) is the owner's own notes — agents never edit it; where shipped
architecture differs, `quran-system.md` Part 5 records why.
