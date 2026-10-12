# G3 — Notification + device surface, audited (round 8, closing pass)

Auditor: G3, notification/device/fan-out pass. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree, HEAD `53d3d421` + uncommitted round-7/8 audit files). Read-only: no repo writes, no DB
writes, no external system contacted. Every `file:line` below was read this session.

**Inputs read first:** `F6-remaining-gaps.md` (the triage that skipped this surface), `F4-observability.md`
(F5 — FCM returns `Ok` with no push), `F3-admin-surface.md` (F12 — suppression kill switch, the unmetered
senders), `F5-token-lifecycle.md` (the FCM device-lifecycle precedent, F1/F8/F10), `A3-repo-integration.md`,
`D5-final-gaps.md` item 3, `E1-incident-response.md` §4.3, `E2-mail-abuse.md` §1.4/§1.5/§2/M8,
`E4-ban-contract.md` (the notice bundle), `C3-base-rate.md` item 19, `C4-shipped-debt.md` items 8/9,
`D1-verification.md` item 12.

**Marking.** `NEW` = the mechanism is not stated in any of the 41 prior reports (checked by grepping all
files for the mechanism, not the topic). `RE-DERIVED` = a prior report states the mechanism; this report
adds evidence, a correction, or a consequence.

**One-line verdict.** The notification/device surface is **not** a user-reachable flood (F6's triage was
right about that) but it is **three live defects and one plan blocker**: (1) the shipped FCM device
endpoint transfers push-token ownership to anyone who knows a token, with no token validation and no
trace on the victim's side; (2) the fan-out is synchronous, unbounded in wall-clock, prunes only one of
four FCM error classes, and returns `Ok` when it delivered nothing — verified exactly as F4 F5 claims;
(3) the mail suppression list is a per-address, never-expiring, unaudited auth kill switch that an
_attacker_ can also trip for a third party's address via the registration → complaint-webhook chain.
The plan blocker: the ban/notice path cannot ship on this mail path until the router reports
_suppressed_ distinctly from _sent_ — today it returns `Ok` and an empty receipt, so the Art 17 notice
would be silently dropped with no record.

---

## 0. Ground truth — the endpoint table (deliverable 1)

Limiter column: **nest** = `rate_limit::rate_limit_layer` on the module nest in `router.rs` (per-IP unit

- matched route pattern); **in-handler** = `abuse_limiter::limiter` inside the handler; **mail** = the
  `MailRouter` buckets that bound any send. Audit = an `audit_logs` row (none exist anywhere — F3 F2).
  All rows below verified this session.

| #   | Endpoint                                  | Method          | Auth (route gate)                                                  | Nest                          | In-handler                        | Mail cap                                    | Audit | Who can act                                    |
| --- | ----------------------------------------- | --------------- | ------------------------------------------------------------------ | ----------------------------- | --------------------------------- | ------------------------------------------- | ----- | ---------------------------------------------- |
| 1   | `/device/v1/register`                     | POST            | `verified` (`device_v1/mod.rs:13`)                                 | 100/min (`router.rs:165-169`) | —                                 | —                                           | no    | any verified user, for themselves              |
| 2   | `/device/v1/list`                         | POST            | `verified`                                                         | 100/min                       | —                                 | —                                           | no    | caller-scoped; **zero web callers**            |
| 3   | `/device/v1/delete`                       | POST            | `verified`                                                         | 100/min                       | —                                 | —                                           | no    | caller's own rows only                         |
| 4   | `/notification/v1/create`                 | POST            | `verified_with_role::<ROLE_ADMIN>` (`notification_v1/mod.rs:9-13`) | 100/min (`router.rs:170-173`) | —                                 | —                                           | no    | **ROLE_ADMIN** only                            |
| 5   | `/notification/v1/list`                   | POST            | `authenticated`                                                    | 100/min                       | —                                 | —                                           | no    | own rows; **zero web callers**                 |
| 6   | `/notification/v1/unread_count`           | POST            | `authenticated`                                                    | 100/min                       | —                                 | —                                           | no    | own rows; zero callers                         |
| 7   | `/notification/v1/mark_read`              | POST            | `authenticated`                                                    | 100/min                       | —                                 | —                                           | no    | own rows; zero callers                         |
| 8   | `/notification/v1/mark_all_read`          | POST            | `authenticated`                                                    | 100/min                       | —                                 | —                                           | no    | own rows; zero callers                         |
| 9   | `/email_verification/v1/verify`           | POST            | `unverified` (**no `not_banned`**)                                 | **none** (`router.rs:82-83`)  | 3/360 s per user → 1 h            | —                                           | no    | any logged-in unverified user                  |
| 10  | `/email_verification/v1/resend`           | POST            | `unverified`                                                       | **none**                      | 3/360 s per user + 1 min delay    | txn 30/h/rcpt                               | no    | same                                           |
| 11  | `/email_verification/v1/admin/issue_code` | POST            | `ROLE_ADMIN`                                                       | **none**                      | **none**                          | txn 30/h/rcpt, provider cap bypassed        | no    | **any Admin, any target**                      |
| 12  | `/email_verification/v1/admin/list`       | POST            | `ROLE_ADMIN`                                                       | **none**                      | —                                 | —                                           | no    | every target's email                           |
| 13  | `/email_verification/v1/admin/delete`     | POST            | `ROLE_ADMIN`                                                       | **none**                      | —                                 | —                                           | no    | destroys the target's pending code             |
| 14  | `/newsletter/v1/subscribe`                | POST            | **public**                                                         | 100/min                       | per-email 5/60 s + per-IP 20/60 s | marketing 5/10 min + provider 50/min, 500/h | no    | anyone, any recipient address                  |
| 15  | `/newsletter/v1/confirm`                  | POST            | **public**                                                         | 100/min                       | —                                 | —                                           | no    | anyone with email+token                        |
| 16  | `/newsletter/v1/unsubscribe`              | POST            | **public**                                                         | 100/min                       | —                                 | —                                           | no    | anyone with email+token                        |
| 17  | `/newsletter/v1/send`                     | POST            | `ROLE_ADMIN`                                                       | 100/min                       | —                                 | marketing + 300 s dedup                     | no    | **all confirmed subscribers**                  |
| 18  | `/newsletter/v1/subscribers/list`         | POST            | `ROLE_ADMIN`                                                       | 100/min                       | —                                 | —                                           | no    | every subscriber email                         |
| 19  | `/mail/v1/suppression`                    | GET/POST/DELETE | `ROLE_ADMIN` (`mail_v1/mod.rs:14-23`)                              | **none** (`router.rs:128`)    | —                                 | —                                           | no    | **any Admin, any address**                     |
| 20  | `/mail/v1/webhook/{provider}`             | POST            | public, HMAC+±5 min (`cloudflare.rs:157-192`)                      | **none**                      | —                                 | —                                           | no    | provider only (secret optional, empty-allowed) |

Two structural facts the table makes visible:

1. **Every nest in this surface except `device_v1` and `notification_v1` has no limiter** — F3 F9's ten-nest
   list is confirmed. The new detail: the two _admin_ mail senders (`admin/issue_code`, suppression CRUD)
   live on the two unmetered nests, so the operator-side blast radius is bounded only by the mail router's
   per-recipient bucket.
2. **No endpoint in this surface writes an audit row** (the writer does not exist — F3 F2). That includes
   the two surfaces whose whole purpose is to act on another user's account: suppression add/remove and
   `admin/issue_code`.

---

## 1. Device register (audit item 1)

### 1.1 What the code does, exactly

`register` (`device_v1/controller.rs:35-62`):

1. `auth.user_required()` — session must be authenticated + verified + not banned (`mod.rs:13`).
2. `ensure_expected_owner(user.id, payload.expected_user_id)` (`:41`, impl `:88-94`) — **fails only if the
   client's asserted owner id differs from the session's user id**. It is a client-consistency check.
3. `device::Entity::upsert` (`db/sea_models/device/actions.rs:9-37`) — `INSERT … ON CONFLICT (token) DO
UPDATE SET user_id = excluded.user_id, platform, updated_at, last_seen_at`. The UNIQUE index is
   `idx_devices_token_owner` (m000011:20-23).
4. `evict_beyond_cap` (`:20-31`) — list the caller's rows `created_at DESC`, delete everything from index
   20 on; **best-effort** (`:53-59` — a failure only logs).

### 1.2 The ownership question, answered honestly

- **Can an attacker register a token they do not own?** Yes, if they know the token string. The server has
  no way to verify token ownership — the token is a bearer string, and the session only proves _who is
  claiming_, never _who the token belongs to_. `expected_user_id` cannot close this: it is compared to the
  caller's own session id, so it is a guard against a _stale client_, not against a hostile one. (D5 item 3
  states "token hijack requires knowing a token"; the mechanism — the guard is self-referential — is NEW.)
- **Can the attacker guess a token?** Not practically: real FCM registration tokens are ~152–163 chars of
  high-entropy base64url. **But the server never checks the shape.** `validator.rs:8` accepts
  `length(min = 1, max = 4096)` and nothing else. So:
  - an attacker can register a 4096-char arbitrary string (self-inflicted: it makes _their_ fan-out fail
    forever, and every admin-created notification pays a failing FCM round trip for it);
  - a _legitimate_ token is accepted with no format sanity check, so a truncated/corrupted token (a real
    client bug class) is stored and retried forever (see §4.3);
  - the stored value is arbitrary attacker bytes, up to 4 KB × 20 rows = 80 KB per account, and it is
    echoed back by `list` (§2).
- **What does a malicious registration achieve?** Bounded, but real:
  1. **Silent denial of the victim's own pushes.** The upsert _moves_ the row; the victim's device count
     drops by one and their account stops delivering to that browser. No notice, no row on the victim's
     side, no audit anywhere.
  2. **Attacker-controlled content on the victim's device** — but only _generic_ content. The push body is
     `"Open EasyQuran to view notifications."` (`services/notification/mod.rs:137-148`) and the only
     producer is an Admin (§3). A normal user has no notification producer at all, so the attacker cannot
     make the victim's phone say anything they choose. (This is the one place the generic-payload rule pays
     off: push hijack without a producer is a nuisance + notification DoS, not data theft.)
  3. **Attribution poisoning of the future evidence layer** — F5 F1's finding, restated as a _shipped_
     defect: the same table that a device-evidence query would read has "whoever registered last owns it".
  4. **A weak existence oracle.** `upsert` uses `exec_with_returning`, so the response `{id}` is the
     _existing_ row's id when the token was already registered and a fresh AUTOINCREMENT id when it was
     not. An attacker holding a candidate token list can therefore test membership by id magnitude. Not
     useful against 152-char random tokens; worth noting only because the endpoint leaks more than it needs.
- **Unstable ownership (NEW).** The web client re-registers on every `visibilitychange` while subscribed
  (`web/src/lib/stores/notifications.svelte.ts:181-185` → `#refreshToken` → `registerTokenWithServer`), so
  a co-user on a shared browser silently takes the token back the next time the page is focused. The
  transfer is a flip-flop, not a settlement — which means the row's `created_at` history is meaningless and
  any future "first seen" reading of it is wrong.
- **The transfer is intended for delivery** (A2 fact 16, B2 fact 15, B5 §1.1, F5 F1) — the failure is that
  the _evidence/abuse_ consumer of this table does not exist yet, and when it lands it will inherit a
  table that silently reassigns ownership. F5 F1's fix (link rows) is the right shape; this report adds
  that the _live_ endpoint also needs a token-shape guard and an explicit transfer record, because the
  FCM table is not going away.

### 1.3 The cap and its eviction policy

- `MAX_DEVICES_PER_USER = 20` (`:17`), eviction = oldest `created_at` first (correct for delivery).
- **Non-atomic.** Check-then-delete across two statements with no transaction (`:24-28`): two concurrent
  registrations can each list 21 and each delete the same tail, or interleave so the count lands at 21–22.
  With the 100/min nest limiter and the pool of 1 the overshoot is small (a handful of rows), so this is a
  correctness wart, not a bypass. The bigger point: **eviction is best-effort** (`:53-59`), so a failed
  eviction leaves the account permanently above the cap, and the fan-out then loops all of them.
- **No expiry.** `devices` has no `expires_at` (C4 item 8 / D1 item 12). Rows live until FCM says
  `UNREGISTERED` or the cap evicts them. With the FCM error classes as they are (§4.3), "until FCM says
  UNREGISTERED" is narrower than it sounds.

---

## 2. Device delete / list (audit item 2)

- **Delete** (`controller.rs:77-86`): `ensure_expected_owner` + `delete_for_user(user.id, token)` →
  `DELETE … WHERE user_id = ? AND token = ?` (`device/actions.rs:48-56`). **Ownership is verified; there is
  no IDOR.** A caller cannot delete another user's row (the `user_id` filter is in the SQL). The only
  observable is `{removed: 0|1}`, which does not distinguish "not yours" from "not found" — correct.
- **List** (`controller.rs:66-73`): `list_for_user(user.id)` → caller-scoped, no IDOR. **But it returns
  `device::Model`, not `DeviceListItem`** — i.e. `id`, `user_id`, `token`, `platform`, `created_at`,
  `updated_at`, `last_seen_at` (`device/model.rs:6-15`). The raw FCM token is echoed back to the caller.
  `DeviceListItem` (`device/slice.rs:11-18`) exists and also carries the token, so the DTO is not a
  ready-made fix — it must be trimmed. Exposure requires a session compromise (caller-scoped), so this is
  Low; it still violates least-data for an endpoint with no client.
- **Zero callers confirmed.** `grep -rn "device/v1" web/src` → only `messaging.ts:142` (register/delete via
  the `postDeviceEndpoint` helper) and its test. Nothing calls `/device/v1/list`. F5 F8's "dormant
  endpoint" is verified. D1 item 12's correction (the Notifications toggle _is_ a user-facing revoke for
  the current device) is also verified: `notifications.svelte.ts:151-156,316-321` → `unregisterTokenFromServer`
  - `deleteFcmToken`.

---

## 3. Notification create (audit item 3)

- **Who can create:** `ROLE_ADMIN` only (`notification_v1/mod.rs:9-13`), synchronous in the handler
  (`controller.rs:83-107`). `auth.user.unwrap()` at `:88` discards the caller entirely — no rank check, no
  self-check, no audit. An Admin can target a SuperAdmin, a peer Admin, or any user id.
- **Can a user create notifications for another user?** No. There is no user-facing producer: `notify_user`
  and `create_and_dispatch` have exactly one call site (`controller.rs:92`), and the web has no producer at
  all. F6's triage ("admin-only producer, no user flood") is **confirmed**.
- **Is the content sanitized?** No, and it does not need to be for text rendering: `title` ≤256,
  `body` ≤2048 (`validator.rs:25-28`), `data` = arbitrary `serde_json::Value` (`:29`). The web toast
  renders `title`/`body` as text and only ever receives the _foreground_ payload after `publicPushPayload`
  strips everything but generic strings (`push-privacy.ts:7-13`). The stored `data` blob is never rendered
  (no inbox UI — §5.2). So: no XSS path today; the free-form `data` is a stored-JSON surface with no
  consumer contract, which is exactly what makes it dangerous the day an inbox ships.
- **Per-recipient cap:** none. `notification::Entity::create` is an unconditional insert
  (`notification/actions.rs:11-27`). 100/min × 60 × 24 = 144 k rows/day for one recipient, ~2.5 KB each at
  max body ⇒ ~360 MB/day, plus up to 20 pushes per call (2 000 pushes/min). D5 item 3 states the missing
  cap; this report adds the numbers and the read-path consequence below.
- **Read-path consequence (NEW).** `mark_all_read` (`notification/actions.rs:78-93`) loads _every_ unread
  row into memory and issues one `UPDATE` per row — each its own implicit transaction, i.e. one fsync per
  row under WAL + `synchronous=FULL` (A7). 144 k unread rows = 144 k serialized commits on the pool of 1
  (≈950 commits/s ceiling per B6 F9 ⇒ ~150 s of stalled writes). `unread_count` (`:51-58`) counts with
  `read_at IS NULL` filtered off an index that does not include `read_at`
  (`idx_notifications_user_created`, m000005), so it is an index scan per call. Today only the _victim_ can
  trigger this and no UI calls it — so it is a latent amplification, not a live one. It becomes live the
  moment the notice path adds a read surface.
- **Non-existent target:** `user_id` has an FK to `users` with CASCADE (m000005), so a bogus id raises a
  DB error → 500 rather than 404. Cosmetic.

---

## 4. The fan-out — F4's claim, verified line by line (audit item 4)

### 4.1 `Ok` with no push: **CONFIRMED, four ways**

`dispatch` (`services/notification/mod.rs:54-135`) inserts the in-app row first (`:63-73`), then:

| Path                                                                         | Lines      | Result                                     |
| ---------------------------------------------------------------------------- | ---------- | ------------------------------------------ |
| FCM client `None` (the default: `FCM_ENABLED` defaults false, `main.rs:424`) | `:75-82`   | `warn!` + `Ok(model)`                      |
| device load fails                                                            | `:84-95`   | `warn!` + `Ok(model)`                      |
| device list empty (the normal case: no web client ever registered a token)   | `:97-99`   | **`Ok(model)`, not even a log**            |
| per-device send fails (429 quota, 5xx, network, timeout)                     | `:124-130` | `warn!`, device kept, loop continues       |
| per-device `Unregistered`                                                    | `:111-123` | prune + continue                           |
| all sends succeed                                                            | `:104-110` | `debug!` only                              |
| **function return**                                                          | `:134`     | `Ok(model)` — always, after the row insert |

So `Ok` means "the notification row was written", never "a push was delivered". F4 F5 is exact. The
return value carries no device count, no success count, no failure count — a caller _cannot_ tell
"pushed to 3 devices" from "no device registered" from "FCM is off" without reading logs.

### 4.2 Synchronous in the request path: **CONFIRMED**

`controller.rs:92-100` awaits `create_and_dispatch`; `dispatch` awaits `client.send(msg)` per device
(`:103`). Nothing is spawned. The web has a precedent for the right shape (`newsletter_v1/controller.rs:164`
uses `tokio::spawn`), so this is an omission, not a policy.

**Does it hold the single DB connection?** **No — and F6/F4's framing should be corrected on this point.**
sea-orm acquires/releases a pooled connection per query (`list_for_user`, the prune-on-unregistered), and
the HTTPS sends happen with no connection held. The failure mode is therefore _not_ pool starvation; it is:

- the admin's request is blocked for the whole fan-out;
- each `Unregistered` prune is an extra commit that interleaves with the 10 s rate-limit flush on the same
  single connection (A7/B6: the flush holds it 60–200 ms);
- **no server-side timeout anywhere.** The shared client is `connect_timeout 5 s` + `timeout 15 s`
  (`state.rs:16-23`), and there is no axum timeout layer and no Traefik response timeout
  (`docker-compose.yml` has only healthcheck timeouts). So 20 devices × 15 s = **~5 minutes** worst case
  for one admin POST, plus the first `mint_access_token` round trip (cached after that, `auth.rs:88-161`).

### 4.3 Error classification: one terminal class out of four is pruned

`FcmClient::send` (`crates/rux-fcm/src/client.rs:45-79`) maps:

- 200/201 → parse `name` (an `Api(200, …)` error if `name` is missing).
- **404 or body containing `UNREGISTERED` → `FcmError::Unregistered`** (`:74-76`) → the fan-out prunes.
- everything else → `FcmError::Api(status, body)` → logged, **device left in place** (`:124-130`).

Unpruned terminal classes, all of which FCM returns for a permanently-dead token:
`400 INVALID_ARGUMENT` (malformed/expired token), `403 SENDER_ID_MISMATCH` (token from another Firebase
project), and `400/401` auth-shaped failures. Also unpruned: a 200 whose body lacks `name`. Consequence:
one bad token costs one failing HTTPS round trip on **every** future notification for that user, forever —
and because the cap counts rows, up to 20 of them. C4 item 8 notes "pruned only when FCM returns
UNREGISTERED"; the INVALID_ARGUMENT/SENDER_ID_MISMATCH classes and the permanent-retry cost are **NEW**.

**Retry / backoff / batch cap / timeout:** none. No retry on 429 (quota) or 5xx; no per-device backoff; no
fan-out batch cap beyond the (non-atomic, best-effort) 20-row cap; no `tokio::time::timeout` around the
loop; no idempotency key, so an admin retry after a timeout re-pushes to every device that already got it
(the row is duplicated too).

### 4.4 What 20 devices actually does

20 sequential HTTPS calls. Each: cached bearer (no network) + one POST ≤15 s. With a healthy FCM, ~20 ×
50–200 ms ≈ 1–4 s — tolerable. With a degraded FCM (or one dead token class returning 403 quickly), it is
20 × 15 s of a blocked admin request. The in-app row is already committed before the loop, so a client
disconnect mid-fan-out leaves rows written and pushes partially delivered with no record.

---

## 5. Push privacy (audit item 5) — **VERIFIED, and the strongest part of this surface**

### 5.1 The payload carries no identifiers

`private_push_hint` (`services/notification/mod.rs:137-148`) is the **only** `FcmMessage` construction in
the repo (grep over `src/` + `crates/`). It sets:

```rust
notification: Some(Notification { title: Some("EasyQuran"), body: Some("Open EasyQuran to view notifications.") }),
data: None, android: None, webpush: None,
```

and the unit test pins the serialized shape (`:154-167`). So the wire payload contains **only the device
token and two constant strings**. No notification id, no user id, no kind, no deep link.

### 5.2 The browser side keeps it that way

- **Service worker** (`web/src/service-worker.ts:953-963`): the `push` handler ignores `event.data`
  entirely and calls `showNotification(PUSH_TITLE, { body: PUSH_BODY, tag: PUSH_TAG, data: { url: "/account" } })`.
  Constants come from `push-privacy.ts:3-5`. The click handler only trusts `notification.data.url` through
  `safeTarget` (same-origin only, `:940-951`) and the value it reads is the one the handler itself wrote.
- **Foreground** (`messaging.ts:49-59` → `publicPushPayload`): strips the payload to `from`/`collapseKey`/
  `messageId` + the constant title/body, then dispatches it; the toast
  (`NotificationToast.svelte:31-60`) renders `title`/`body` as text and would render `data.url` as an
  `href` — but `data` is `{}` after the scrub, so the branch is dead.
- **No private data reaches a push payload.** Bookmarks, reading position, and comment/message content
  never appear: there is no producer that could put them there (§3), and the one push builder is constant.
  The residual risk is structural, not live: the guarantee is enforced by _one function being the only
  constructor_, with no guard test that it stays that way. Add a source guard (§9 P2).

**Caveat worth stating in the plan:** the in-app row (`notifications.title/body/data`) _does_ contain
whatever an admin wrote, and it is returned to the recipient by `/notification/v1/list`. That is the
correct place for content; it just must never be forwarded to FCM. It is not today.

---

## 6. Email verification (audit item 6)

### 6.1 `admin/issue_code` — the unmetered sender

`email_verification_v1/controller.rs:248-286`:

1. `user::Entity::get_by_id(user_id)` — **no rank check**, so any Admin can target a SuperAdmin.
2. `generate_code()` (8 chars, `email_verification/model.rs:39-45`) + HMAC (`utils/code_hash.rs:7-11`).
3. `regenerate_for_email` (`email_verification/actions.rs:98-113`) — **overwrites the existing row's
   `code_hash` and `updated_at`**. There is **no `is_in_delay()` check** on this path (unlike `resend`,
   `:73-88`), so the admin path bypasses the 1-minute delay entirely.
4. `send_email_verification_code` (`services/mail/mod.rs:34-50`, `TEMPLATE_VERIFICATION`) → transactional.

Consequences:

- **Mail-bomb any address, bounded by 30/h/recipient.** F3 F12 says "at line rate"; the accurate shape is
  **30 sends/hour to one address** (the transactional per-recipient bucket, `router.rs:33-40`), **and the
  provider-wide bucket is deliberately skipped for transactional mail** (`router.rs:141-157`), so _N_
  distinct targets give _N_ × 30/h with no global ceiling (D5 1b's missing global cap, now with the
  admin-side trigger named). **Correction to F3 F12.**
- **Denial of verification (NEW).** Each call invalidates whatever code the user just requested. An admin
  (or a compromised admin session) can keep any unverified account from ever completing verification, and
  the user has no way to tell why: `resend` will keep minting codes, but the admin's next call kills them.
  Low direct impact, high nuisance; and it is a _silent_ action on another account with no audit row.
- **No nest limiter** (`router.rs:82-83` has no layer on this nest), so the endpoint itself is unmetered;
  only the mail router slows it.
- **No audit row.** F3 F2 lists `email_verification_v1::{admin_issue_code, admin_delete}` — confirmed.

### 6.2 `resend` limits, entropy, expiry, brute force

- `resend` (`:65-142`): 1-minute delay per row (`DELAY_TIME`, `model.rs:36`), then the abuse limiter keyed
  `email_verification:{user_id}` with `temp_block_attempts 3 / range 360 / duration 3600` (`:29-36`).
- **Attempt semantics (NEW detail).** `InMemoryStore::abuse_check` (`rux-request-gate/src/store.rs:287-350`)
  appends an attempt on _every_ call and blocks when `short_count >= temp_block_attempts` — so the 3rd call
  within 360 s is already blocked, i.e. **2 guesses per window, then a 1 h temp block**, and the limiter is
  evaluated _before_ the code check (`controller.rs:48-54`). Attempts are pruned at `block_range` (900 s),
  so after each 1 h block the counter resets; the 24 h `Long` block is effectively unreachable on this key
  (temp is evaluated first and re-trips). Effective ceiling ≈ 3 guesses/hour ≈ 72/day.
- **Entropy/expiry:** 8 chars of `Alphanumeric` = 62^8 ≈ 2.18 × 10^14, `EXPIRY_TIME = 3 h`
  (`model.rs:36-45`), generated with `rand::rng()` (thread RNG, OS-seeded). Sound.
- **Verdict:** brute force is not a finding. `verify_code` (`actions.rs:63-96`) is a single
  `DELETE … WHERE user_id AND code_hash AND updated_at > now-3h` in a transaction, then sets `is_verified`
  only if the email still matches — correct.
- **Two real gaps:**
  1. **`unverified` omits `not_banned()`** (`middlewares/auth_guard.rs:46-55` vs `:57-70`). A banned user
     with an unverified account keeps the verify + resend path (and the mail). Low severity; a
     consistency bug in the guard set.
  2. **The limiter is skipped outside production** (`abuse_limiter.rs:39-52`: `if !cfg!(test) &&
matches!(is_production(), Ok(false)) { return Ok(()) }`). This is deliberate and fail-closed on an
     unset/unknown `APP_ENV` (the `matches!` is false ⇒ the limiter runs). Correct as written; worth
     stating in the plan so a staging box is not mistaken for "the limiter is broken".

---

## 7. Newsletter (audit item 7)

### 7.1 Subscribe — arbitrary recipient, double opt-in that cannot complete

`subscribe` (`newsletter_v1/controller.rs:55-124`):

- Public, `#[validate(email)]` only, per-email 5/60 s + per-IP 20/60 s + nest 100/min. E2 §1.5 verified all
  of this.
- **The table does not exist** (C3 item 19 / D5 1e): `newsletter_subscribers` has no migration
  (`grep -rn newsletter rust/backend/api/migration/src/` → 0 hits) and no runtime
  `CREATE TABLE IF NOT EXISTS` (the only four are `sessions`, `rate_limit_state`,
  `translation_popularity`, `user_oauth_identities`), and it is absent from the dev DB table list. So
  `SubscriberEntity::create` fails and the endpoint 500s **before** the send. Confirmed.
- **The confirm URL points at a route that does not exist (NEW).** `:101-106` builds
  `{site_url}/newsletter/confirm#email=…&token=…`. `grep -rln newsletter web/src/` → **zero files**; there
  is no `/newsletter/**` route anywhere in the app. The fragment is client-side by design ("fragments
  aren't sent to servers"), which means the _only_ way it could work is a page that reads
  `location.hash` and POSTs to `/newsletter/v1/confirm` — that page does not exist. **So even after C3
  F8's table fix, the double opt-in can never complete, and the subscriber can never unsubscribe** (there
  is no unsubscribe page either, and no `List-Unsubscribe` header — E2 M8 / C4 item 9). This is a
  plan-level activation blocker, not a bug in a dormant path.
- **Duplicate subscribe downgrades a Confirmed subscriber (RE-DERIVED: C4 item 9 states the mechanism).**
  `Entity::create` (`newsletter_subscriber/actions.rs:15-66`) finds by email and, if found, **overwrites
  `token` and `status` with the new Pending values**. So anyone who knows a subscriber's address can
  silently reset their subscription to Pending (they stop receiving the newsletter) and trigger a fresh
  confirmation mail. C4 §9 says "subscribe re-issues the token, status → Pending"; the _unauthenticated
  third-party_ consequence and the mail-flood shape (up to the marketing bucket) are the new part.

### 7.2 Confirm / unsubscribe

- Both are POST, public, nest-limited 100/min, no in-handler limiter.
- Token compare is **constant-time** (`actions.rs:77-79`, `:108-113`) — good.
- Token is **stored plaintext** (`newsletter_subscriber/model.rs:13`) — C4 item 9 / E2 F-5.
- `confirm` refuses only `Unsubscribed` (`:80`), so a Confirmed row can be re-confirmed; `unsubscribe`
  requires email+token, so a lost confirmation mail = a subscriber who can never unsubscribe (E2 F-5).
- **No token oracle:** `confirm`/`unsubscribe` return the same `SubscriberNotFound` for a wrong token and a
  missing row. Correct.

### 7.3 Send — admin-only, uncapped, uncancellable, silently no-op today

`send` (`:154-239`): ROLE_ADMIN, nest 100/min, then `tokio::spawn` a loop over **all** Confirmed
subscribers with a 1.3 s pace, retry ≤2 on `Throttled`, and log a summary. Findings:

- **Silent 202 (NEW).** The handler returns `202 {"message":"Newsletter send queued"}` **before** the task
  touches the DB. Today the task's first query fails (missing table) and only an `error!` line records it;
  the admin sees "queued". The same shape means: after the table lands, a send to a broken provider, or a
  send that dies halfway, is invisible to the admin. There is no job row, no progress, no cancel, no
  idempotency (D5 1e / F3 §1.9 name the missing cap/cancel/confirm; the silent-202 is new).
- **Unbounded body (NEW).** `V1SendNewsletterPayload` validates `subject` ≤200 and `text` min 1 — **no
  max** on `text` or `html` (`validator.rs:22-29`). Axum's default `Json` body limit is 2 MB here (no
  `DefaultBodyLimit` on this nest), so one admin request can carry ~2 MB of body, which is then re-sent to
  every subscriber and hashed for the dedup key. A 10 k-subscriber list ⇒ ~20 GB of outbound API traffic
  from one POST.
- **Content is admin-authored** and goes to the provider as `html`/`text` verbatim. No sanitization, no
  footer enforcement, no unsubscribe link (E2 F-5's fix covers the header + footer).
- **Dedup** is content-hash + 300 s (`mail/router.rs:262-274`) — it prevents an accidental double-click,
  not a deliberate resend with a changed subject.

### 7.4 Subscriber list

Admin-only, returns emails (`SubscriberListItem`), no audit (F3 F11's read set). F3 F11 covers it.

---

## 8. Mail suppression as a kill switch (audit item 8) — **CONFIRMED, with two NEW mechanisms**

### 8.1 The switch itself

- **Who can add:** `ROLE_ADMIN` (`mail_v1/mod.rs:14-23`). No rank check, no required reason
  (`V1CreateSuppression` has `reason: Option` and no `Validate` attributes at all — `mail_v1/validator.rs:19-27`;
  the handler takes bare `Json(payload)`, so even if attributes existed they would not run — `controller.rs:43-46`).
  No nest limiter (`router.rs:128`). No audit row.
- **What it does:** `MailRouter::send` → `lookup_suppressed` (`router.rs:119-135`) → `suppressed` ⇒ for a
  transactional template (`TEMPLATE_VERIFICATION` / `TEMPLATE_PASSWORD_RESET`, `provider.rs:17-22`):
  `debug!` + `Ok(SendReceipt::default())` (`router.rs:251-259`). The caller cannot distinguish it from a
  real send. **So suppressing an address silently disables both verification and password reset for that
  account.** F3 F12's claim is exact.
- **Matching rule:** `row.permanent || (row.reason == Bounce && now - last_seen <= 86 400)` (`:132-134`).
  A complaint is forced permanent by the upsert (`actions.rs:36,41-48`); a _soft_ bounce suppresses for
  24 h; sync bounces are written `permanent: true` unconditionally (`router.rs:206-221` — E2 §2 gap 2).
- **Reversible:** yes — `DELETE /mail/v1/suppression?recipient=` (`controller.rs:66-77`), by exact
  recipient string, no id, no confirmation. No audit on either direction.
- **Blast radius:** any Admin can lock any address out of account recovery, permanently, silently, with no
  record and no notification. F3 §8.1 item 7 already lists this as a rogue-admin move; the missing
  _attacker_ path is below.

### 8.2 NEW: an attacker can permanently suppress a third party's address

Chain, each link verified:

1. `POST /auth/v1/register` is public and takes an **arbitrary submitted address**
   (`auth_v1/controller.rs:459-535`, spawn at `:503-521`); on success it sends the verification mail to that address in a
   detached task (`:504-517`). E2 §2 line 141 records the sender; the _suppression consequence_ is not
   stated in any report.
2. The recipient (a real third party) marks the unsolicited mail as spam, or their server bounces it.
3. The provider webhook fires; `apply_mail_event` (`mail_v1/controller.rs:143-156`) upserts a **complaint
   with `permanent: true`** (or a bounce with the provider's permanence flag).
4. `lookup_suppressed` now returns true for that address **forever**, and every transactional send to it
   returns `Ok` with nothing sent.

Net: **an unauthenticated attacker can permanently disable password reset for any address they know, by
typing it into the registration form and waiting for the owner to mark the mail as spam.** The address is
the key; the account is irrelevant; no admin action is involved. This works only when
`CLOUDFLARE_EMAIL_WEBHOOK_SECRET` is configured (E2 §2 gap 1: it is in no env example and no deploy step,
so today the complaint path is dead and only the admin switch exists). Severity is conditional on the
webhook, which is precisely the fix E2 F-1 asks for — so **fixing the webhook activates this**.
`NEW`.

**Second NEW mechanism — the switch is exact-string and alias-blind.** `canonicalize_recipient`
(`router.rs:319-351`) only trims and lowercases; it does not strip `+tags` or Gmail dots. So
`victim+news@example.com` and `victim@example.com` are different suppression keys for the same mailbox.
Direction matters: it does _not_ let a mailer bypass a suppressed address (all app mail goes to the
account's stored address), but it does mean an operator who suppresses `victim@gmail.com` will still mail
`victim+x@gmail.com` if that is the stored address, and it makes "is this address suppressed?" a
string-equality question rather than a mailbox question. `NEW`.

**Third detail — the kill switch is one-directional and unaudited.** Nothing reads the suppression list to
_inform_ the user, and `delete_suppression` returns `{removed}` (a presence oracle for the caller, who is
already an admin). The `source` column does record `admin` vs `mail-webhook` vs `{provider}-send-sync`
(`actions.rs:84`), which is the minimum needed for a review policy — it is not used by anything.

### 8.3 The coupling the notice path inherits

The ban/notice path (E1 §4.3, E4) will send a **transactional** notice mail. Two consequences:

1. If the notice uses a template **not** in `is_transactional`, a suppressed address gets
   `Err(MailError::Suppressed)` (`router.rs:258`) — loud, but the notice fails. If it _is_ transactional
   (E1's fix adds `TEMPLATE_ENFORCEMENT_NOTICE` to `is_transactional`), the suppressed address gets `Ok`
   and an empty receipt — silent. Either way **the caller cannot record `notice_delivery_state`** without
   a router change, because the suppression fact is discarded inside `send`. E1 §4.3 says "change the
   enforcement path to write `notice_delivery_state='suppressed'`"; this report adds the required
   prerequisite: **the router must return a distinguishable outcome first** (§9 P0.1).
2. E1's `notice_delivery_state` enum is `sent|suppressed|no_contact_details|pending`; E4's DDL comment is
   `'sent' | 'no_contact_details' | 'send_failed'`. **The two ban-contract designs disagree**; `suppressed`
   is the state that matters here. Flagged as a cross-report correction.

---

## 9. The fix set, ordered (deliverable 3), then attacked

Ordering rule: the notice path's mail dependency first (it is the plan blocker), then the surfaces that
can act on another user's account, then the availability/abuse limits, then hygiene.

### P0 — before the ban/notice path ships

| #   | Fix                                                                 | Exact change                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| --- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Make suppression legible to callers**                             | `MailRouter::send` returns a distinguishable outcome: add `SendOutcome { Sent(SendReceipt), Suppressed, Throttled, Failed(MailError) }` (or `SendReceipt { suppressed: bool, .. }`). Keep the _response body_ anti-enumeration identical for auth callers — only the internal return value changes. Then the notice path can write `notice_delivery_state='suppressed'`. Evidence: `router.rs:251-259`, `:258`.                                                                                                                                         |
| 2   | **Suppression governance**                                          | `mail_v1`: require `reason` + a rank check (target account's address must belong to a strictly-lower role, or an explicit `force` flag) + an `audit_logs` row (`mail.suppression.add`/`.remove`) in the same transaction. Add the nest limiter (60/min) at `router.rs:128`. Make `permanent` complaint/bounce rows **review-required before they block transactional mail**: add `blocks_transactional INTEGER NOT NULL DEFAULT 1` (0 for webhook-sourced rows) so a complaint cannot silently kill account recovery; a one-line admin action flips it. |
| 3   | **Fan-out: spawn, cap, budget, record**                             | Move the loop out of the request: `tokio::spawn` the fan-out (newsletter precedent, `newsletter_v1/controller.rs:164`), wrap it in `tokio::time::timeout(Duration::from_secs(10))`, and write one row recording `{user_id, notification_id, devices, sent, failed, pruned, elapsed_ms}` (the `job_runs` table F4 proposes, or the existing `audit_logs` with `action='notification.dispatch'`). Return the device count in the 201 body so the admin sees "0 devices" immediately.                                                                      |
| 4   | **Prune terminal FCM errors**                                       | In `rux-fcm/src/client.rs`, classify 400/`INVALID_ARGUMENT`, 403/`SENDER_ID_MISMATCH`, 404/`UNREGISTERED` as terminal; in `dispatch`, mark the device (`invalid_at` column) and prune after 2 consecutive terminal failures (a 400 can be transient during an FCM incident). Keep 429/5xx as transient with one retry + jitter.                                                                                                                                                                                                                         |
| 5   | **Notification per-recipient cap + single-statement mark_all_read** | Cap on insert (e.g. keep the newest 200 unread / 1000 total; evict oldest) **with an explicit exemption for notice/enforcement kinds**; replace the loop in `notification/actions.rs:78-93` with one `UPDATE notifications SET read_at=? WHERE user_id=? AND read_at IS NULL`; add a retention row (C4's schedule).                                                                                                                                                                                                                                     |
| 6   | **Device register: token shape + transfer trace**                   | `validator.rs`: token `length(min = 64, max = 512)` + charset `[A-Za-z0-9_-]` (FCM tokens are base64url-ish; do **not** regex-pin the version prefix — tokens change shape). Keep the transfer for the shared-browser case, but make it visible: write an `audit_logs` row (`device.transfer`, `{token_hash_prefix, from_user_id, to_user_id}` — never the raw token) and return `transferred_from` in the response.                                                                                                                                    |
| 7   | **Audit writer**                                                    | F3 P0 #1 (same-transaction writes). Wire `notification.create`, `device.register`/`transfer`/`delete`, `mail.suppression.*`, `email_verification.admin_issue_code`. Without it none of the above is reviewable.                                                                                                                                                                                                                                                                                                                                         |

### P1 — next

| #   | Fix                           | Change                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 8   | `admin/issue_code` hardening  | Add the nest limiter; add `target.role < caller_level`; honour `DELAY_TIME` (or record the override); don't clobber a code the user requested < 60 s ago (or record the actor + reason); audit.                                                                                                                                                                                      |
| 9   | Newsletter activation bundle  | Table (C3 F8) + **web confirm page** + **one-click unsubscribe route + `List-Unsubscribe`/`List-Unsubscribe-Post` headers** (E2 F-5) + `text`/`html` length caps (64 KB) + a persisted job row with an id so 202 means something + never downgrade a Confirmed subscriber on duplicate subscribe (return the same 201 and keep Confirmed, or require an explicit re-subscribe flag). |
| 10  | `unverified` guard            | Add `.not_banned()` (`auth_guard.rs:46-55`), and reflect it in E4's banned-UX classifier copy.                                                                                                                                                                                                                                                                                       |
| 11  | Device list DTO + atomic cap  | Return a trimmed DTO (`id`, `platform`, `created_at`, `last_seen_at` — no token, no `user_id`); make the cap one `DELETE … WHERE id IN (SELECT id FROM devices WHERE user_id=? ORDER BY created_at DESC LIMIT -1 OFFSET 20)` inside the same transaction as the upsert, and fail the registration if it fails.                                                                       |
| 12  | Mail webhook activation guard | Requiring the secret (E2 F-1) must ship **with** #2's review-required rule, or it activates the third-party suppression chain (§8.2).                                                                                                                                                                                                                                                |

### P2 — defer, with the trigger

| Fix                                                               | Defer reason / trigger                                                                                                                                              |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A Devices section in settings (F5 §1.5) + `device/v1/list` wiring | Needs the evidence-token work; until then, either delete the dormant endpoint or make it return the trimmed DTO (#11). Trigger: the first user-visible device list. |
| Device retention/expiry (C4 item 8)                               | Ships with the retention sweeper (E1 F7); the `invalid_at` column from #4 is the right seed.                                                                        |
| A push-privacy source guard                                       | One test: any `rux_fcm::FcmMessage` construction outside `private_push_hint` fails the build. Cheap, but it guards a property that is currently correct.            |
| Per-admin rate limits on the operator nests                       | Subsumed by #2 + F3 P0 #5 (nest limiters).                                                                                                                          |

### 9.1 Attack my own fixes

1. **Spawning the fan-out moves the failure out of the request** — the admin gets 201 and never learns the
   push failed. That is exactly F4 F5's problem, reintroduced one layer down. **Mitigation is mandatory:**
   #3's result row must be written and the 201 body must carry the device count; a `devices=0` response is
   the loud signal. Without the row, do not spawn.
2. **Pruning on 400 could delete a live device.** FCM can return 400 for reasons other than a dead token
   (malformed message, project misconfig) — and then _every_ device 400s and a naive prune wipes all of
   them. **Mitigation:** 2-strike rule + a global circuit breaker (if >50 % of devices fail with the same
   class in one fan-out, mark nothing and alert). Never prune on 403 without the same 2-strike rule.
3. **The notification cap could swallow a notice.** A cap that evicts or drops rows will eventually drop an
   enforcement notice — an Art 17 violation caused by a spam control. **Mitigation:** the cap must exempt
   notice kinds explicitly and must never delete an unread notice; cap by _count of non-notice rows_, or
   refuse (429) rather than evict when the exempt class is full.
4. **Token shape validation can reject a real token.** FCM has changed token formats before; a charset or
   length rule that is too tight silently disables push for a cohort. **Mitigation:** validate length +
   charset only (64–512, base64url), log every rejection with the reason, and ship it behind the same
   kill-switch pattern as the rest (`FCM_TOKEN_VALIDATION_ENABLED`, default on, one env line to disable).
5. **Reject-on-conflict (instead of transfer) breaks the shared-browser case** the web store relies on
   (`onAuthChanged` tears down, then re-registers; a failed teardown leaves the token owned by the previous
   account). **Mitigation:** do not reject; keep the transfer and make it visible (#6). The web store's
   re-register then _correctly_ flips ownership back.
6. **Review-required complaint suppressions can re-enable sending to a complained address** — the one send
   class that damages domain reputation (E2 §1.4). **Mitigation:** scope the exemption to _transactional_
   templates only (E4's template-keyed rule), keep marketing blocked by default, and surface the count in
   the digest (F4 Q7) so the operator sees a suppressed address receiving account-recovery mail.
7. **`not_banned()` on `unverified` changes the banned-UX.** A banned unverified user gets 403 instead of
   the verify screen; E4's classifier must map it (and the copy must not leak "banned" to a user who is
   merely unverified).
8. **The single-statement `mark_all_read` is one long write on the pool of 1.** 144 k rows in one
   transaction holds the connection for the duration and blocks the 10 s flush. **Mitigation:** the cap
   (#5) is what makes it bounded; also chunk it (`LIMIT 500` per statement in a loop with `yield_now`), or
   accept it once the cap holds the ceiling at ~1000.
9. **Removing the token from the list DTO could break a future client.** Verified zero callers
   (`grep -rn "device/v1" web/src`), so no client breaks today; the fix must ship with a note that the
   token is not part of the response contract.
10. **`SendOutcome` changes every mail caller's match arms** — `resend`, `forgot_password::generate`,
    `admin_issue_code`, `register` (spawned), newsletter. **Mitigation:** keep `MailRouter::send` returning
    `Result<SendReceipt, MailError>` and add `SendReceipt.suppressed: bool` (a struct field, not a new
    enum variant) so existing `?`-based callers compile unchanged and only the notice path reads the flag.
    The anti-enumeration contract stays in the _response_, not in the return type.

---

## 10. The minimum required before the ban/notice path ships (deliverable 4)

The notice's dependency chain, with the exact minimum at each link. **Everything below is required; none
of it is optional for an Art 17-shaped notice.**

1. **The mail router must report suppression** (§9 P0.1). Today a suppressed transactional send is
   byte-identical to a delivered one from the caller's perspective (`router.rs:251-259`). Without this, the
   notice row cannot record `suppressed` and the design cannot distinguish "notified" from "we sent
   nothing". **This is the single blocking item.**
2. **The suppression list must be governed** (§9 P0.2): audited add/remove, a rank check, and
   webhook-sourced permanent rows must not block transactional mail unreviewed — otherwise the notice is
   silently dropped for any user who once marked a verification mail as spam, and §8.2's attacker chain is
   a third-party notice killer. The `suppressed` outcome from (1) must be recorded on the notice row, and
   the fallback is E1 §4.3's public appeal URL (the only channel a suspended user can reach).
3. **The mail path itself must be provably alive**: `MAIL_PROVIDER` set, `MAIL_RATE_LIMIT_ENABLED=true`
   (default), the per-recipient transactional bucket (30/h) not exhausted by the admin's own
   `issue_code`/resend loops, and a tested send. A notice that depends on a bucket another admin action
   can exhaust is not a notice.
4. **`TEMPLATE_ENFORCEMENT_NOTICE` must be added to `is_transactional`** (`provider.rs:17-22`) and the
   suppression rule must key on the template (E4 §24-28's correction). Add the template + Tera file
   (E1 §4.3).
5. **The in-app channel must not be load-bearing.** `notify_user` returns `Ok` with no push in four ways
   (§4.1), the inbox endpoints have zero web callers (§5.2), and the ban revokes sessions (E4 step: session
   kill) so the user may never see the in-app row. **Minimum:** either ship an inbox surface first, or
   declare the notice channel = mail + public appeal URL and record `notice_channel='email'`; do not claim
   `'in_app'` as delivered.
6. **The notification cap must exist _with_ the notice exemption before `notify_user` gets its first
   automated caller** (§9 P0.5, attack #3). Today the fan-out has zero automated producers, so the missing
   cap is dormant; the ban/notice path is the first producer.
7. **The audit writer** (F3 P0.1) must cover the notice + suppression + dispatch rows. A notice without an
   audit row is the same defect class as a ban without a notice.
8. **A dispatch-result record** (§9 P0.3/#4): the operator must be able to answer "did the notice reach
   anyone" without reading container logs. F4's digest slot 1 (`notices_undelivered > 24 h`) depends on it.

**What can wait:** device retention/expiry, the Devices settings UI, the evidence-token work, the
newsletter activation (independent of the notice), and the FCM error-class expansion beyond
`UNREGISTERED` (nice, not blocking).

---

## 11. Findings (deliverable 5)

All `NEW` unless marked. Severity is this surface's severity, not the app-wide severity.

### N1 — Device register accepts an arbitrary unvalidated token and the upsert silently transfers any known token to the caller; `expected_user_id` is a client-consistency check, not an ownership proof. `NEW` (the transfer itself is D5 item 3 / F5 F1; the guard's self-referential nature, the missing shape validation, and the victim-silent flip-flop are new).

**Severity: Medium.**
**Evidence:** `device_v1/controller.rs:41,88-94` (`ensure_expected_owner` compares the caller's session id
to the client's claim), `:42-50` (upsert); `db/sea_models/device/actions.rs:21-34`
(`ON CONFLICT(token) DO UPDATE SET user_id = excluded.user_id`); `device_v1/validator.rs:8`
(`length(min=1, max=4096)`, no charset); `device_v1/mod.rs:13` (`verified` — any verified user);
`m000011:20-23` (the UNIQUE index that makes the transfer work);
`web/src/lib/stores/notifications.svelte.ts:181-185` (re-register on visibilitychange ⇒ ownership flips
back). D5 item 3: "token hijack requires knowing a token"; F5 F1 pins the transfer as intended behaviour.
**Fix:** §9 P0.6 (shape guard + transfer audit + `transferred_from`). Do not reject on conflict.

### N2 — Only `UNREGISTERED`/404 is pruned; `INVALID_ARGUMENT` (400) and `SENDER_ID_MISMATCH` (403) are retained forever, so one dead token makes every future fan-out pay a failing round trip. `NEW` (C4 item 8 notes the UNREGISTERED-only prune; the classes and the permanent-retry cost are new).

**Severity: Medium.**
**Evidence:** `crates/rux-fcm/src/client.rs:73-78` (404/`UNREGISTERED` → `Unregistered`; everything else →
`Api(status, body)`); `services/notification/mod.rs:111-123` (prune on `Unregistered` only), `:124-130`
(other errors: `warn!`, device kept). F4 F5's `Ok` claim is verified at `:134`.
**Fix:** §9 P0.4 (2-strike terminal classification + a global circuit breaker).

### N3 — The fan-out is synchronous in the admin request path with no batch cap, no retry, and no wall-clock budget: 20 devices × the 15 s HTTP timeout ≈ 5 minutes per admin POST, and the response is 201 regardless. `NEW` (F4 F5 covers the `Ok`; the synchronous blocking, the timeout arithmetic, and the missing server-side timeout are not stated anywhere).

**Severity: Medium-High.**
**Evidence:** `notification_v1/controller.rs:90-100` (awaited in the handler); `services/notification/mod.rs:101-132`
(sequential per-device `.await`); `state.rs:16-23` (5 s connect / 15 s total); `docker-compose.yml`
(no Traefik response timeout; only healthcheck timeouts); `router.rs:170-173` (100/min per IP).
**Correction to F6/F4's framing:** it does **not** hold the pooled connection during the HTTPS sends
(sea-orm releases per query); the failure is a blocked request + interleaved prune commits, not pool
starvation.
**Fix:** §9 P0.3.

### N4 — No per-recipient notification cap and no retention, and `mark_all_read` rewrites every unread row one commit at a time. `NEW` (D5 item 3 states the missing cap; the N-commit read-path amplification and the numbers are new).

**Severity: Medium.**
**Evidence:** `notification/actions.rs:11-27` (unconditional insert), `:78-93` (load-all + per-row
`UPDATE`); `notification_v1/validator.rs:21-29` (title 256 / body 2048 / free-form `data`);
`router.rs:170-173` (100/min ⇒ 144 k rows/day/recipient); `m000005` (index on `(user_id, created_at)`,
none on `read_at`); A7 (WAL + FULL ⇒ one fsync per commit).
**Fix:** §9 P0.5 (cap with a notice exemption + one-statement mark_all_read + retention).

### N5 — The notification inbox has zero web callers, so the in-app half of the future enforcement notice has no surface, and the push that _does_ arrive opens a generic `/account` page. `NEW` (the dormant `device/v1/list` is F5 F8; the dormant _notification_ inbox and its notice-path consequence are new).

**Severity: Medium (plan-relevant).**
**Evidence:** `grep -rn "notification/v1" web/src` → 0 hits (only `device/v1` in `messaging.ts` + tests);
`web/src/service-worker.ts:953-963` (`data: { url: "/account" }`);
`web/src/lib/components/notifications/NotificationToast.svelte:31-60` (renders only the scrubbed
foreground payload); `services/notification/mod.rs:137-148`.
**Fix:** §10 item 5 — declare the notice channel or ship the surface; do not count `in_app` as delivered.

### N6 — `admin/issue_code` is unmetered at the nest, has no rank check, skips the resend delay, invalidates the target's pending code, and can send 30 mails/hour to any address with the provider-wide cap bypassed. `NEW` (F3 F12 states the mail-bomb and the missing rank check; the code invalidation, the 30/h precision, and the transactional provider-cap bypass interaction are new).

**Severity: Medium-High.**
**Evidence:** `email_verification_v1/controller.rs:248-286` (no rank check at `:255-260`, no
`is_in_delay` check, `regenerate_for_email` at `:264-270`); `email_verification/actions.rs:98-113`
(overwrites `code_hash` + `updated_at`); `router.rs:82-83` (no nest limiter);
`mail/router.rs:33-40` (txn 30/h/rcpt), `:141-157` (`if !transactional` skips the provider bucket).
**Correction to F3 F12:** "at line rate" → 30/h per recipient, uncapped across recipients.
**Fix:** §9 P1.8.

### N7 — The newsletter confirmation link points at a web route that does not exist, and there is no unsubscribe page or header: the double opt-in can never complete and a subscriber can never unsubscribe. `NEW` (E2 F-5/C4 item 9 cover the missing `List-Unsubscribe` and the plaintext token; the missing web route is in no report).

**Severity: Medium-High (compliance + activation blocker).**
**Evidence:** `newsletter_v1/controller.rs:101-113` (`{site_url}/newsletter/confirm#email=…&token=…`);
`grep -rln newsletter web/src/` → **0 files** (no `/newsletter/**` route anywhere);
`grep -rn "List-Unsubscribe" rust/backend/api/src web/src` → 0 hits.
**Fix:** §9 P1.9 (web confirm page + one-click unsubscribe + headers, shipped with the table).

### N8 — `newsletter/v1/send` returns 202 before touching the DB (today: a silent no-op with a DB error in the logs), has no job record/cancel/progress, and accepts an unbounded `text`/`html` body that is re-sent to every subscriber. `NEW` (D5 1e / F3 §1.9 name the missing cap/cancel/confirm; the silent-202 and the unbounded body are new).

**Severity: Medium.**
**Evidence:** `newsletter_v1/controller.rs:154-239` (202 at `:235-238`; `tokio::spawn` at `:164`);
`newsletter_v1/validator.rs:22-29` (`text` min 1, no max; `html` unvalidated);
`router.rs:139-142` (100/min); `m000005` + `grep newsletter migration/src` → no table.
**Fix:** §9 P1.9.

### N9 — The mail suppression list is an exact-string, never-expiring, unaudited kill switch; any Admin can suppress any address with no reason, and an **unauthenticated attacker can permanently suppress a third party's address** through the registration → complaint-webhook chain. `NEW` (F3 F12 states the admin kill switch; the attacker-triggerable third-party suppression, the alias-blind key, and the `permanent` complaint semantics are new).

**Severity: High** (conditional on `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` being configured; the webhook is
E2 F-1's recommended fix, so the fix activates this).
**Evidence:** `mail_v1/controller.rs:43-64` (no reason/rank/audit), `:143-156` (complaint ⇒
`permanent: true`); `mail_v1/validator.rs:19-27` (no validation attributes; handler takes bare `Json`);
`mail_v1/mod.rs:14-23` (ROLE_ADMIN); `router.rs:128` (no nest limiter);
`email_suppression/actions.rs:32-71` (never-downgrade upsert), `:73-90` (trim+lowercase only);
`mail/router.rs:119-135` (the `permanent || bounce-within-24h` rule), `:243-259` (suppressed
transactional → `Ok`), `:319-351` (no alias folding); `auth_v1/controller.rs:459-535` (register sends to
an arbitrary address); E2 §2 gap 1 (webhook secret in no env example).
**Fix:** §9 P0.2 + P1.12 (review-required webhook rows; alias folding decision; audit + rank + reason).

### N10 — `device/v1/list` returns the raw FCM token and the full row; `delete` is correctly ownership-scoped; the cap is a non-atomic check-then-delete with best-effort eviction. `NEW` (F5 F8 notes the zero callers; the token-in-response and the cap's non-atomicity are new).

**Severity: Low.**
**Evidence:** `device_v1/controller.rs:66-73` (returns `Vec<device::Model>`), `:20-31` (list-then-delete),
`:53-59` (best-effort evict); `device/model.rs:6-15` (Serialize includes `token`, `user_id`);
`device/actions.rs:48-56` (`WHERE user_id AND token` — no IDOR).
**Fix:** §9 P1.11 (trimmed DTO; one-statement atomic cap; fail the registration on eviction failure).

### N11 — `notification/v1/create` has no rank check (an Admin can push-spam a SuperAdmin) and writes no audit row; the response is 201 even when zero pushes happened. `NEW` (F3 F2 covers the missing audit; the rank angle and the 201-regardless response are new).

**Severity: Low-Medium.**
**Evidence:** `notification_v1/controller.rs:83-107` (`let _admin = auth.user.unwrap()` at `:88`;
`Ok((StatusCode::CREATED, …))` at `:107`); `notification_v1/mod.rs:9-13`;
`services/notification/mod.rs:134` (always `Ok`).
**Fix:** §9 P0.3 (device count in the response) + `target.role <= caller_level` + the audit row.

### N12 — The `unverified` guard omits `not_banned()`, so a banned user with an unverified account keeps the verification mail path. `NEW`.

**Severity: Low.**
**Evidence:** `middlewares/auth_guard.rs:46-55` (`authenticated().unverified()`) vs `:57-70`
(`verified()` adds `.not_banned()`).
**Fix:** §9 P1.10.

### Verified non-findings (recorded so the next reader does not re-litigate)

- **Push privacy is sound.** The only `FcmMessage` construction is constant-text with `data: None`
  (`services/notification/mod.rs:137-148`, pinned by `:154-167`); the SW ignores `event.data` and shows
  constants (`service-worker.ts:953-963`); the foreground scrub strips everything
  (`push-privacy.ts:7-13`). **No private data reaches a push payload.** F6's claim holds.
- **No user-reachable notification flood exists.** `notify_user` has one call site, admin-gated
  (`notification_v1/controller.rs:92`); the web has no producer. F6's triage holds — the missing cap is
  dormant until the first automated producer (the notice path).
- **Device delete/list have no IDOR.** Caller-scoped SQL (`device/actions.rs:48-56`, `:39-46`).
- **Brute-forcing verification codes is not viable.** 62^8 entropy, 3 h expiry, 2 guesses per 360 s then a
  1 h temp block, evaluated before the code check (§6.2).
- **The newsletter token compare is constant-time** (`newsletter_subscriber/actions.rs:77-79,108-113`),
  and the mail recipient canonicalizer blocks header injection (`mail/router.rs:319-351`, tests `:432-451`).

---

## 12. Corrections to prior reports

1. **F3 F12's "at line rate" for `admin/issue_code`** → the mail router caps transactional mail at
   **30/hour per recipient** (`mail/router.rs:33-40`); the provider-wide bucket is skipped for transactional
   templates (`:141-157`), so the correct statement is "30/h/recipient, uncapped across recipients, on an
   unmetered nest".
2. **F6 §2/F4's "blocking the single DB connection" framing** → the fan-out blocks the _request_, not the
   pooled connection (sea-orm acquires per query). The pool-of-1 cost is the interleaved prune commits and
   the flush contention, not a held connection.
3. **E1 §4.3 vs E4's DDL comment disagree on the notice delivery-state enum** (`sent|suppressed|…` vs
   `sent|no_contact_details|send_failed`). `suppressed` is the state the mail path can actually produce and
   the one Art 17 needs; the two documents must be reconciled, and the mail router must return the fact
   first (§9 P0.1).
4. **C4 item 8's prune statement is correct but incomplete** — `INVALID_ARGUMENT` (400) and
   `SENDER_ID_MISMATCH` (403) are also terminal and are never pruned (N2).
5. **D5 item 3's "token hijack requires knowing a token" is correct**, but the endpoint adds no shape
   validation and no transfer trace (N1); "knowing a token" includes a co-user's `localStorage` on a shared
   browser (the FCM record is in `localStorage`, not HttpOnly — `notifications.svelte.ts:22`).

---

## 13. Verification commands (all read-only)

```bash
# endpoint table / limits
sed -n '80,84p;125,133p;139,177p' rust/backend/api/src/router.rs
sed -n '8,14p'   rust/backend/api/src/modules/device_v1/mod.rs
sed -n '8,23p'   rust/backend/api/src/modules/notification_v1/mod.rs
sed -n '8,23p'   rust/backend/api/src/modules/email_verification_v1/mod.rs
sed -n '8,22p'   rust/backend/api/src/modules/newsletter_v1/mod.rs
sed -n '13,33p'  rust/backend/api/src/modules/mail_v1/mod.rs
sed -n '46,55p;57,70p' rust/backend/api/src/middlewares/auth_guard.rs

# device register/transfer/cap
sed -n '16,94p'  rust/backend/api/src/modules/device_v1/controller.rs
sed -n '1,12p'   rust/backend/api/src/modules/device_v1/validator.rs
sed -n '9,66p'   rust/backend/api/src/db/sea_models/device/actions.rs
sed -n '10,25p'  rust/backend/api/migration/src/m000011_device_token_ownership.rs

# fan-out (the F4 F5 verification)
sed -n '54,148p' rust/backend/api/src/services/notification/mod.rs
sed -n '45,79p'  rust/backend/api/crates/rux-fcm/src/client.rs
sed -n '16,23p'  rust/backend/api/src/state.rs
grep -n "timeout" docker-compose.yml                    # healthcheck only

# notification storage
sed -n '11,27p;78,93p' rust/backend/api/src/db/sea_models/notification/actions.rs
sed -n '6,20p'   rust/backend/api/migration/src/m000005_device_notification_oauth.rs

# email verification
sed -n '40,61p;65,142p;248,286p' rust/backend/api/src/modules/email_verification_v1/controller.rs
sed -n '63,113p' rust/backend/api/src/db/sea_models/email_verification/actions.rs
sed -n '35,46p'  rust/backend/api/src/db/sea_models/email_verification/model.rs
sed -n '287,350p' rust/backend/api/crates/rux-request-gate/src/store.rs   # attempt semantics
sed -n '39,52p'  rust/backend/api/src/services/abuse_limiter.rs

# newsletter
sed -n '55,124p;128,150p;154,239p' rust/backend/api/src/modules/newsletter_v1/controller.rs
sed -n '15,66p;68,123p' rust/backend/api/src/db/sea_models/newsletter_subscriber/actions.rs
sed -n '22,29p'  rust/backend/api/src/modules/newsletter_v1/validator.rs
grep -rn "newsletter" rust/backend/api/migration/src/    # 0 hits
grep -rln "newsletter" web/src/                          # 0 files
grep -rn "List-Unsubscribe" rust/backend/api/src web/src # 0 hits

# suppression kill switch
sed -n '43,77p;120,165p' rust/backend/api/src/modules/mail_v1/controller.rs
sed -n '32,98p'  rust/backend/api/src/db/sea_models/email_suppression/actions.rs
sed -n '119,159p;206,221p;243,282p;319,351p' rust/backend/api/src/services/mail/router.rs
sed -n '17,22p'  rust/backend/api/src/services/mail/provider.rs
sed -n '459,535p' rust/backend/api/src/modules/auth_v1/controller.rs   # register -> arbitrary recipient
sed -n '80,81p'  rust/backend/api/src/main.rs                         # webhook secret empty-allowed

# push privacy
sed -n '137,167p' rust/backend/api/src/services/notification/mod.rs
sed -n '1,14p'   web/src/lib/firebase/push-privacy.ts
sed -n '940,983p' web/src/service-worker.ts
sed -n '93,168p' web/src/lib/firebase/messaging.ts
grep -rn "device/v1\|notification/v1" web/src            # device register/delete only
```

_End of G3._
