# Incident response runbook

Operator runbook for abuse reports, P0 alerts, the daily digest, and the drill.
Companion: `deploy/moderation-sop.md` (ban decision checklist). Legal open items:
`deploy/legal-debt.md`.

Owner steps still open: O12 (operator FCM device), O13 (drill), O19 (uptime monitor).

## Configuration

| Variable | Effect when unset |
| --- | --- |
| `OPERATOR_ALERT_EMAIL` | No P0 mail, no digest mail. `alert.dispatch` records `mail.status = not_configured`. |
| `OPERATOR_USER_ID` | No P0 push. Record shows `push.status = not_configured`. |
| `FCM_ENABLED`, `FCM_PROJECT_ID`, `FCM_SERVICE_ACCOUNT_PATH` | No push to any device. Operator push needs these plus `OPERATOR_USER_ID`. |
| `MAIL_PROVIDER` (not `none`) | Mail is dropped; `NoOpMailProvider` returns no receipt, so the alert records `not_accepted`. |
| `ABUSE_BAN_WRITES_ENABLED` | Ban routes are not mounted. Needed to act on a P0 with a ban. |
| `IMAGE_MODERATION_ENABLED` + `IMAGE_MODERATION_URL` | Off. In `APP_ENV=production` the API refuses to boot while media upload is mounted and this pair is unset. |

`OPERATOR_USER_ID` is the account the operator signs into on the phone. That account must
register a device (O12) for push to reach anyone.

## What "accepted" means

- Mail `accepted`: the mail provider returned a receipt with `delivered` or `queued`.
- Push `accepted_by_fcm`: FCM returned a message name for at least one device.
- Neither label means the operator read the alert. Check the `reports` table on a schedule
  until the digest and alert paths have been seen working end to end.
- Push text is generic ("Open EasyQuran to view notifications."). It names no report.
- Mail carries the report id and priority only. It carries no content and no reporter.

## Where to look

- Every P0 intake writes one `audit_logs` row, `action = alert.dispatch`, `resource_id` =
  report id, `metadata` = `{priority, mail: {status}, push: {status, accepted, failed}}`.
  A missing row after a P0 means the dispatch did not run: check the api log for
  `P0 alert dispatch result not audited`.
- Digest: one `job_runs` row per 08:00 UTC run, `job = operator_digest`, `detail` =
  `open_p0=N open_p1=N open_appeals=N mail=<status>`. A missing row for a day means the
  scheduler did not run that day: check the api log.
- Health: `GET /healthz` (returns `healthy` or `degraded` from the DB check). The
  external uptime monitor (O19) polls this path. Owner sets it up.

## P0 runbook (csam and threat reports)

1. Alert arrives (mail, push, or both). If neither arrives, read the `alert.dispatch` row
   for the report id. Act on the report from the database even if the alert failed.
2. Read the report row on the api host (read-only query on `reports`: `id`, `target_kind`,
   `target_id`, `reason`, `priority`, `status`, `snapshot`, `created_at`). The snapshot
   holds the content captured at intake. Operators can also list open reports with
   `GET /admin/reports/v1?status=open&priority=p0` (session admin, read-only).
3. Preserve evidence before any removal. For a comment report, on the api host:
   `just incident capture <report-id>` (A25). It copies the comment into `case_evidence`
   and writes one audit row. Legal holds: `just legal-hold place --rule <class> --authority <t>
   --reason <t> --owner <t> --subject <kind:id>` or `POST /admin/legal-holds/v1` (admin session).
   Each place and renew writes a `legal_hold.*` audit row; `just legal-hold list` shows holds.
4. Hide or remove the content through the existing comment or post controls. Each action
   writes its own audit row.
5. Account action, if the evidence supports one: follow `deploy/moderation-sop.md`
   (checklist, reason code, `sharedDeviceChecked`, `singleOperatorControl` for renewals).
   Create via `POST /admin/users/bans` (requires `ABUSE_BAN_WRITES_ENABLED`). Check
   `deliveryState` in the response. `sent` means the provider accepted; anything else
   needs follow-up with the resend route.
6. CSAM: the code does not refer material to any authority. The referral step is a
   legal and operational duty outside this build. See `deploy/legal-debt.md` item 1.
7. Set the review status: `POST /admin/reports/v1/{id}/status` with `{"status": "reviewing" |
   "actioned" | "dismissed"}` (admin session). Each change writes one `report.status_set`
   audit row. Open P0 and P1 counts in the digest fall only when the status leaves `open`.
8. Record the case note: steps taken, times, reason code, who decided.

## Upload screening and CSAM triage (A37)

What the code does, automatically:

- Every media upload runs moderation (`IMAGE_MODERATION_URL`), then known-hash match, both
  before dedup. A flagged image, a known-hash match, or a provider error refuses the upload.
  Nothing is stored for a refused image.
- A refusal writes one audit row: `media.moderation_reject` or `media.hash_match`, with
  `resource_id` = SHA-256 of the image bytes and `user_id` = uploader. No bytes are kept.
- Known-hash matching has no provider in this build. The refusal path exists; the match
  source does not, so it is inactive until the owner registers a provider (O29).
- A known-hash match sends one operator alert by mail (`EasyQuran: known-hash match blocked`),
  fire-and-forget after the refusal. It names no image, uploader or hash. Its result is one
  `alert.dispatch` audit row with `priority = hash_match` and the mail status. A moderator
  refusal (`media.moderation_reject`) raises no alert: check `audit_logs` for it daily.

Triage. Human-confirmed violating content only. The code never removes stored content.

1. A human confirms the content violates policy. A match alone is not a decision. Quran
   text is never violating content.
2. Preserve before any removal: place an A25 legal hold on the media id
   (`just legal-hold place --rule enforcement_12m --subject media:<id> ...` or
   `POST /admin/legal-holds/v1`). A held item survives erasure and is listed in the receipt.
   Record the hold id.
3. Remove: `DELETE /media/v1/{id}` (A30 takedown: storage objects, CDN purge when
   `CDN_PURGE_ZONE_ID` and `CDN_PURGE_API_TOKEN` are set, tombstone and audit row).
4. Report to NCMEC: outside this build. The code makes no referral. Owner (O29) and
   `deploy/legal-debt.md` item 1.
5. Freeze the account: L6 ban via `POST /admin/users/bans` per `deploy/moderation-sop.md`
   (human decision before effect; needs `ABUSE_BAN_WRITES_ENABLED`).
6. Case note: times, who confirmed, hold id, tombstone id, report id if any.

## Appeals

- Appeal intake: `POST /appeal/v1/submit`, behind `ABUSE_BAN_WRITES_ENABLED`, one per
  enforcement notice (A35). Report intake is `POST /report/v1/submit`. Decision
  clock: 7 days (`DECISION_CLOCK_SECS`). The digest's `open_appeals` counts every open
  appeal; overdue ones are in `appeals.due_at`.
- Decision: `POST /admin/appeals/v1/{id}/decision` with `{"decision": "upheld" | "overturned"}`
  (admin session, open appeals only). It writes one `appeal.decision` audit row and stops the
  decision clock: the appeal leaves `open`, so it no longer counts as overdue or open. The
  decision does not change the ban. If the appeal is overturned, revoke the ban through
  `POST /admin/users/bans/{id}/revoke`, a separate audited step. Open appeals:
  `GET /admin/appeals/v1`.

## Drill (owner runs, O13)

The drill is owner-executed. No agent ran it. Run it end to end once and log it below.

1. Submit a test P0 report in a test environment (`MAIL_PROVIDER` real or a test mailbox,
   `OPERATOR_ALERT_EMAIL` and `OPERATOR_USER_ID` set, FCM on with the operator device).
2. Confirm the `alert.dispatch` row shows the mail and push statuses. Confirm the mail arrives.
3. Capture evidence (`just incident capture`), then ban (ladder L6) through the ban route.
4. Confirm the notice `deliveryState`. Resend if needed.
5. Submit an appeal against the notice. Confirm the appeal row.
6. Record the decision in the case note. Revoke or confirm the ban.
7. Log the run below. Set the next drill date.

### Drill log

| Date | Operator | Report id | Mail status | Push status | Notice state | Appeal | Decision | Gaps found |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| _none yet_ | | | | | | | | |

Next drill date: _not set_ (owner sets it).
