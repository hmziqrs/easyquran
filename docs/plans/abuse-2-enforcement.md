# Abuse phase 2: ban contract and enforcement

[Back to index](../README.md)

#### A38. Erasure and data-request procedure

Build: admin erase path writing an erasure receipt (A33), one-month response clock with queue-aging alert, runbook section in `deploy/legal-debt.md`. Self-service deletion is F7 and writes the same receipt.

Files: `rust/backend/api/src/modules/user_v1/`, `rust/backend/api/src/services/retention.rs`, `deploy/legal-debt.md`.

Done when: an admin erase leaves evidence under hold intact, deletes the rest, and writes a receipt (test); overdue requests alert.

Blocked by: A26, A33

#### A34. Policy text sentence ledger, minors, code side of legal artifacts

Build:

1. Sentence ledger: for each sentence in `web/src/routes/(marketing)/**` privacy and terms, record the code reference that makes it true (table in `deploy/legal-debt.md`, or a comment map). Remove sentences without a reference. After A32 is green, publish the full privacy policy. Abuse device-identity paragraph only if it ships (not in this queue); D2 adds the account-device sentence.
2. Minors: self-declared date of birth → `users.is_minor` flag (migration). For minors: no device records, no push, no analytics, no comments. India: no accounts under 18 and no device processing (default; O6 confirms). Test the minor flag path end to end.
3. No global consent banner. Consent is jurisdiction-aware. If client collection ever ships, just-in-time consent at login, equal-weight decline, reader never gated.
4. Art 17 statement-of-reasons template and appeal SOP text: in `deploy/moderation-sop.md` (A22 creates it).
5. Legal artifacts (DPIA, LIA, ROPA, retention schedule): owner authors and signs (O11). Do not write them here. Code side only: `deploy/legal/` is the owner's.

Files: `web/src/routes/(marketing)/**`, `web/messages/{en,ar}.json`, `rust/backend/api/src/db/sea_models/user/*`, `rust/backend/api/src/modules/user_v1/*`, new migration for `is_minor`, `deploy/legal-debt.md`.

Done when: sentence ledger has no unreferenced sentences (reviewer check); minor flag test blocks device write, push, analytics, and comments for minors; `bun run check`, `bun run lint`, `bun run test` green.
Blocked by: A4, A26, A32
Owner: O11, O6.
