# Abuse-prevention audit reports (raw evidence)

These 59 reports are the evidence base for [`docs/abuse-prevention.md`](../../abuse-prevention.md). They were produced by an adversarial audit loop: independent agents attacked a draft design, the design was revised, and the revised design was attacked again until a dedicated exhaustion pass (`H1-exhaustion.md`) concluded that another round would only add detail.

**Precedence rules — read these before citing anything here.**

1. **`docs/abuse-prevention.md` is authoritative.** These reports disagree with each other; later reports correct earlier ones. Where they conflict, the plan is correct and the reports are not.
2. **§16.1 of the plan lists claims that were corrected and must not be repeated.** Numbers, mechanisms, and guarantees appear in these reports that were later refuted. Do not copy a number out of a report without checking that list.
3. **Numbers are labelled by provenance in the plan's §16.2** (measured / sourced / estimated). Reports do not consistently label them. An estimate presented as a fact in a report is still an estimate.
4. **Every claim here was true of the code at `53d3d421`.** Anything read as current fact must be re-verified against the code before it is used.
5. These are working documents, not a specification. They contain rejected designs (`DESIGN-v0.md`, `DESIGN-v1.md`), attack reports, and meta-audits. The rejected designs are kept deliberately: the plan's §15 lists what was rejected and why, and these files are the evidence for those rejections.

**Do not implement from these files.** Implement from `docs/abuse-prevention.md`, which states the normative rules, the acceptance criteria, and the gates.

## Inventory

| Group          | Files | Topic                                                                                                                                                                                                                                                     |
| -------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `R2-*`         | 7     | External research: ban operations, evasion countermeasures, fingerprint signal science, commercial products, network/transport fingerprinting, legal and privacy, session binding and risk-based auth                                                     |
| `A1–A10`       | 10    | Attacks on the first design draft: threat model, false positives, repo integration, web/cache collisions, privacy and legal, testability, performance and cost, ops realism, crypto and keys, data model                                                  |
| `B1–B10`       | 10    | Token storage decision, contrarian option comparison, anonymous/reader abuse, detection-rule engineering, shared-device design, rollout sequencing, repo drift and unguarded invariants, retention execution, second-order attack, research falsification |
| `C1–C6`        | 6     | Option A+B audit, contradiction resolution by code, abuse base-rate measurement, shipped legal debt, decision ledger, plan shape                                                                                                                          |
| `D1–D5`        | 5     | Claim verification, recommended-scope attack, measurement-plan attack, hard-rule compliance, final gap sweep                                                                                                                                              |
| `E1–E6`        | 6     | Incident response, mail abuse, billing, ban contract, fresh-eyes audit, reader-path cost                                                                                                                                                                  |
| `F1–F6`        | 6     | Ingress trust, auth availability, admin surface, observability and kill switches, device-token lifecycle, remaining-gap enumeration                                                                                                                       |
| `G1–G4`        | 4     | Media pipeline, sync and UGC bounds, device endpoints and fan-out, pre-mortem                                                                                                                                                                             |
| `H1–H3`        | 3     | Exhaustion verdict, production reachability verdicts, final consistency sweep                                                                                                                                                                             |
| `DESIGN-v0/v1` | 2     | The rejected draft designs, kept as the record of what was ruled out                                                                                                                                                                                      |

## Loop outcome

The loop ran 10 rounds across 63 agent sessions. It closed on evidence, not on a budget: `H1-exhaustion.md` maps every module, service, middleware, crate, and route family to a report that read the code, and applies a strict test — a subsystem is only a gap if auditing it could change a plan decision. Every remaining candidate failed that test (detail-only, duplicate, verification pass, or an owner decision). The residual caveat is recorded there too: coverage is not proof of absence.

The direction changed because of the audits. The original request was for a consistent browser fingerprint. The audits established that component fingerprinting is cohort-valued, client-authored, and noised by browser vendors; that the highest-value abuse work in this repo is account- and credential-centric; and that the app could not ban anyone at all. The plan reflects that, with the device layer deferred behind a measurable gate rather than deleted.
