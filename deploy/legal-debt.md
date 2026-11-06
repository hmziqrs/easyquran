# Legal debt

Open legal and compliance items the code does not settle. Each row names the trigger that
makes it due. Do not publish a claim the code does not enforce while its row is open.
Owner decisions live in `docs/plans/owner.md`; this file is the trigger list for them.
Reference: `docs/research/abuse-prevention/R2-legal-privacy.md`.

Status as of this commit: no item is closed.

| # | Item | Owner ref | Trigger (due before) | Status |
| --- | --- | --- | --- | --- |
| 1 | CSAM referral path. Code captures and alerts P0 reports (A24, A26). No automated or manual referral step exists in the build. Hash-match provider and NCMEC registration not done. | O29, A37 | First live P0 csam report, or any image upload path | Open |
| 2 | Named owner for DPIA, LIA, ROPA, retention schedule and quarterly measurement. Unowned until named. | O11 | Any enforcement ban in production | Open |
| 3 | Privacy and terms text written, signed, and dated in `deploy/legal/`. | O11, A34 | Public launch of report or appeal pages | Open |
| 4 | Erasure wording. Default claim: removed from the live database; removed from backups within 30 days. Confirm before publishing. | O15, A33 | Publishing any erasure claim | Open |
| 5 | Custody document: where the pepper and other secrets live, who answers DSARs, what happens on restore or change of control. | O16, A34 | Publishing the privacy text | Open |
| 6 | Minors: no accounts under 18 in India (default, unconfirmed). Decide whether the rule covers the account device list. | O6, A34 | First India signup, or minors path | Open |
| 7 | Appeal decision route. Intake exists (A35). No API records a decision, so decisions live only in case notes. The 7-day decision clock (DECISION_CLOCK_SECS) has no enforcement. | A35 follow-up | First real appeal | Open |
| 8 | Report status change. No route sets `reports.status`; open counts never fall. Retention of reports is 12 months (`report_12m`). | A24 follow-up, A38 | First P0 handled | Open |
| 9 | Store rules for device identity. Android: no persistent device id claim (confirm). Apple: no derived device data (confirm). | O31 | Each first store release | Open |
| 10 | Font and translation licences inside app bundles. Mehr Nastaliq Web is not OFL. | O35 | Native client release (M5, G2, N3) | Open |
| 11 | Recitation licence and source. | O22 | Reciter catalogue or radio (F19, F20) | Open |
| 12 | Licensed word and token source. | O23 | F21 | Open |
| 13 | Licensed tafsir sources. | O24 | F22 | Open |
| 14 | Operator alert mailbox and FCM device: `abuse@` mailbox provisioned and MX verified; operator device registered. | O3, O12 | Before relying on P0 alerts | Open |
| 15 | External uptime monitor on `/healthz`. | O19 | Before relying on the digest or alerts | Open |

## Erasure and data requests (A38)

Clock: one month from `data_requests.received_at` (`due_at`). The `data_request_clock` job runs every 6 h and mails the operator when any open request is past `due_at`. The mail carries counts and days only, no subject ids. Check `job_runs` for `data_request_clock`.

Admin procedure (routes under `/user/v1/admin/`, admin role, all `POST`):

1. Confirm the requester controls the account. The code has no in-app identity check; do it out of band.
2. `data_request/open` with `{"user_id": N}`. Writes `data_request.open` to `audit_logs`. Note `due_at`.
3. `POST data_request/list` shows the open queue, earliest deadline first.
4. Before erasing, check the subject for legal holds (`legal_holds`, `legal_hold_subjects`). A live user hold keeps every user-keyed row and the `users` row. The receipt lists what was kept. Tell the subject what was kept and why (owner wording).
5. `POST data_request/erase/{request_id}` with `{"confirm_email": "..."}`. Returns `receipt_id`, `deleted`, `retained_tables`, `user_row_erased`.
6. Check the `erasure_receipts` row and the closed `data_requests` row.

What the erase does (code: `services/data_request.rs`, `services/retention.rs`):

- Deletes the subject's rows in every user-keyed table not in the evidence list (`ERASE_EVIDENCE`, `ERASE_EXTRA_TABLES`), except rows under a live hold.
- Scrubs the `users` row when no live user hold applies: name, mailbox (set to `erased-<id>@erased.invalid`), password, 2FA, OAuth link, mailbox hash. Rotates the session secret. The id stays.
- Writes the receipt (A33) and closes the request, in one transaction.

What it does not do. Do not publish a claim about these:

- Keeps evidence: `audit_logs`, `user_bans`, `enforcement_notices`, `media_deletions`, `reports`, `case_evidence`.
- Does not delete authored posts or uploaded media or avatar files.
- Does not physically remove data. Rows leave the live database. Backups keep them until the 30-day cap (A33). The public erasure wording stays unpublished until row 4 closes (O15).

Self-service deletion (F7) reuses the same path: `data_request::open` with origin `self`, then `data_request::erase` with the subject as actor. No self-service route exists yet.
