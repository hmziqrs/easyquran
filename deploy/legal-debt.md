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
