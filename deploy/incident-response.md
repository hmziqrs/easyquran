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
   holds the content captured at intake. No admin read route exists in this build.
3. Preserve evidence before any removal. For a comment report, on the api host:
   `just incident capture <report-id>` (A25). It copies the comment into `case_evidence`
   and writes one audit row. Legal holds: `services/legal_hold.rs`.
4. Hide or remove the content through the existing comment or post controls. Each action
   writes its own audit row.
5. Account action, if the evidence supports one: follow `deploy/moderation-sop.md`
   (checklist, reason code, `sharedDeviceChecked`, `singleOperatorControl` for renewals).
   Create via `POST /admin/users/bans` (requires `ABUSE_BAN_WRITES_ENABLED`). Check
   `deliveryState` in the response. `sent` means the provider accepted; anything else
   needs follow-up with the resend route.
6. CSAM: the code does not refer material to any authority. The referral step is a
   legal and operational duty outside this build. See `deploy/legal-debt.md` item 1.
7. Record the case note: steps taken, times, reason code, who decided. No report
   status route exists yet, so `status` stays `open` after handling. Open counts in the
   digest will not drop until a later item adds the status change.

## Appeals

- Appeal intake: `POST /appeal/v1/submit`, behind `ABUSE_BAN_WRITES_ENABLED`, one per
  enforcement notice (A35). Report intake is `POST /report/v1/submit`. Decision
  clock: 7 days (`DECISION_CLOCK_SECS`). The digest's `open_appeals` counts every open
  appeal; overdue ones are in `appeals.due_at`.
- No decision route exists in this build. Record the decision in the case note and the
  audit row, and revoke the ban through `POST /admin/users/bans/{id}/revoke` if it is
  reversed. Flag this gap to the owner before the first real appeal.

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
