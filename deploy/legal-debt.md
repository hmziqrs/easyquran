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

Self-service deletion (F7) reuses the same path: `POST /user/v1/delete` (`user_v1::controller::self_delete_account`) checks fresh auth and the typed email, ends every session, then calls `data_request::erase_self`. Immediate, no grace period. Web entry: `DeleteAccountControls` on the Account page.

## Policy sentence ledger (A34)

Every sentence on `/privacy` and `/terms` and the code that makes it true. A sentence with no row is not published. Legal-only wording (contract terms, no technical claim) is marked `no code claim` and needs owner review (O11). Keys live in `web/messages/{en,ar}.json`.

| ID | Sentence (short) | Code reference | Status |
| --- | --- | --- | --- |
| P-S1 | Analytics and crash reports off unless turned on in Settings | `web/src/lib/stores/consent.svelte.ts` (default `analytics: false`); `boot/analytics.ts`, `boot/crash-reporting.ts` (consent-gated load) | Kept |
| P-S2 | Personal reading insights always on, yours alone | `user_analytics_v1` routes (`auth_guard::verified`, own `user_id`) | Kept |
| P-S3 | Push runs only if you enable it | `web/src/lib/firebase/messaging.ts` `requestPermission`; `device_v1::register` stores token only when client sends one | Kept |
| P-S4 | Bookmarks and reading history sync only when signed in | `bookmark_v1`, `reading_session_v1` (auth + `expectedUserId`); signed-out data stays in `web/src/lib/bookmarks/store.svelte.ts` | Kept |
| P-S5 | Notes never leave this device | No notes table or route in `rust/`; no notes in `web/src/lib/sync` | Kept |
| P-S6 | Nothing sold or used to build an ad profile | None | Removed |
| P-C1 | Analytics and crash reports via Firebase, only after opt-in | `web/src/lib/firebase/analytics.ts`; consent gate as P-S1 | Kept |
| P-C2 | Google sets a browser identifier; sees coarse screens, features, errors | `analytics.ts` `pageView`, `track()` callers (`notifications.svelte.ts`, `locale-suggest.svelte.ts`) | Kept |
| P-C3 | Never records which surahs or verses you read | `web/src/lib/firebase/analytics.ts` (route filter: surah and range routes excluded, comment at line 63). Partly verified: event payloads not audited field by field | Kept, re-check on change |
| P-C4 | Reading insights count verses and time; most-read, time read, resume | `user_analytics_v1` events and summary; `reading_progress`, `reading_daily`, `reading_ayah_totals` | Kept (minors: see K3) |
| P-C5 | Insights shown only to you | `user_analytics_v1` routes scoped to caller | Kept |
| P-C6 | Never sold or used for ads | None | Removed |
| P-A1 | Reading needs no account | Reader routes not behind auth guard | Kept |
| P-A2 | We store email and sign-in details | `users` row (email, password hash, `user_oauth_identities`, `passkey_credentials`) | Kept |
| P-A3 | Bookmarks, folders, history, insights sync across devices | `bookmark_v1`, `reading_session_v1`, `user_analytics_v1` | Kept |
| P-A4 | Notes stay in browser, never synced | As P-S5 | Kept |
| P-A5 | Administrators can reach account data | `admin_*` route modules, role-guarded | Kept |
| P-A6 | Delete your account yourself from Account page | `DeleteAccountControls` (`web/src/routes/(account)/account/+page.svelte`); `POST /user/v1/delete` | Kept |
| P-A7 | Deletion removes live data straight away: sign-in, sessions, devices, bookmarks, history, comments | `self_delete_account` to `data_request::erase_self`; `retention.rs` `erase_tables` (every `user_col` table plus `ERASE_EXTRA_TABLES`; `post_comments` has `user_id`) | Kept |
| P-A8 | Audit, moderation and enforcement records, deletion record and legal holds kept | `retention.rs` `ERASE_EVIDENCE` (`audit_logs`, `user_bans`, `enforcement_notices`, `media_deletions`); `reports`, `case_evidence` have no `user_col`; `erasure_receipts`; holds | Kept |
| P-A9 | Authored posts and uploaded media not removed | Erase path excludes posts and media (A38, `legal-debt.md` above) | Kept |
| P-A10 | Database backups keep copies for 30 days | Backups dark and owner-gated (A33) | Removed (not live) |
| P-D1 | Before sign-in, bookmarks, notes, history, insights stored in browser | Local stores in `web/src/lib/bookmarks/store.svelte.ts` and reader state | Kept |
| P-D2 | Signing in adds the bookmarks on this device to your account | `bookmarks/store.svelte.ts` (sign-in join) | Narrowed (was: and history and insights) |
| P-D3 | Notes stay on this device either way | As P-S5 | Kept |
| P-H1 | Server and CDN handle IP and request details | `client_ip` (`CfConnectingIp`), request handling | Kept |
| P-H2 | Used to run service and limit abuse, incl. rate limits and their records | `rate_limit_layer` on every nest; `rate_limit_state` (m000002) | Kept |
| P-N1 | Enable notifications, we store a browser token | `device_v1::register` token upsert (not for minors, K2) | Kept |
| P-N2 | Turn off in device or browser settings | Platform permission control. No code claim | Kept (platform) |
| P-K1 | Email sign-up asks date of birth; keep only under-18 flag | `auth_v1::register` (`parse_date_of_birth`, `is_minor_on`; DOB not stored); `users.is_minor` (m000200); `RegisterForm.svelte` | Kept (new) |
| P-K2 | Under 18: no device records or push tokens | `device_v1::register` minor branch (no row, no token); `device_v1::issue_challenge` refused for minors | Kept (new) |
| P-K3 | Under 18: no reading events stored | `user_analytics_v1::events` minor branch (`accepted: 0`) | Kept (new). Firebase client analytics NOT gated for minors (see gaps) |
| P-K4 | Under 18: no comments | `post_comment_v1::create` minor refused (403) | Kept (new) |
| P-K5 | Third-party sign-in not asked for date of birth | `user::Entity::create_from_google` leaves `is_minor` false | Kept (new). Gap: O6 |
| P-Ch1 | Use without account, analytics off, decline notifications, delete your own account | P-A1, P-S1, P-N2, P-A6 | Kept |
| P-Ch2 | Clearing browser data removes local data | Browser storage. Platform behaviour | Kept |
| P-G1 | Policy changes update the date at the top | Page `copy.updated` | Kept |
| T-1 | Acceptance and use of the app | No code claim | Kept (owner review, O11) |
| T-2 | Reader, search, bookmarks, personal notes, without warranty | `quran_v1`, `search_v1`, `bookmark_v1`, local notes | Kept |
| T-2b | Free of charge | Billing modules exist (`billing_v1`, plans). Owner to confirm reader pricing | Removed |
| T-2c | Made by oxlabs, run by hmziq.rs | Owner-supplied identity. No code claim | Kept (owner) |
| T-3 | Care with Arabic text; errors possible; rely on printed mushaf | Quran data immutable (`AGENTS.MD`, `db/` provisioning). Advice wording | Kept |
| T-4 | Your notes and bookmarks remain yours; notes never leave device | P-S5 | Kept |
| T-5 | Sync permission to store and transfer bookmarks and sessions, for that feature only | `bookmark_v1`, `reading_session_v1` | Kept |
| T-6 | No licence over content beyond running the service | No code claim | Kept (owner review) |
| T-7 | Conduct rules; no disruption or abusive scraping | `rate_limit_layer` (T-8) | Kept |
| T-8 | Rate limits apply | `rate_limit_layer` on all nests (`router.rs`) | Kept (dropped "to everyone") |
| T-9 | We may remove content or restrict an account when breaking terms or law | `post_comment_v1` admin hide and delete; `user_bans` (L6); `enforcement_notices` | Kept |
| T-10 | Wrong decision: contact us | `appeal_v1` intake (A35) | Kept |
| T-11 | Third-party translations remain holders' property | No code claim. Licences open: `legal-debt.md` rows 10 to 13 | Kept (owner review) |
| T-12 | Provided as is; no liability for loss of local notes or bookmarks | Local-only storage (P-D1) | Kept (owner review) |
| T-13 | Questions to salam@easyquran.app | Owner-supplied mailbox. Response-time promise removed | Kept (owner) |
| T-13b | Respond within a few days | None | Removed |

### Gaps the ledger records (not closed here)

- Firebase client analytics is not gated for minors. Server refuses reading events and device rows, but a minor who turns on Settings analytics still sends Firebase events. Fix needs client gating on the minor flag, or the minor sentence narrowed further.
- `create_from_google` (Google, Apple, GitHub, Facebook OAuth creates) never sets the minor flag. Owner decision O6 (India) and an OAuth date-of-birth step are open.
- Existing accounts read `is_minor = 0`. No backfill. Flag only applies to accounts created after m000200.
- Erasure wording (row 4, O15) still open. Privacy page says "straight away"; keep only while the self path stays immediate.
