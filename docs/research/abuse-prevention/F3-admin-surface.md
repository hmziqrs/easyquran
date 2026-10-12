# F3 — The admin surface, audited as a whole (round 7)

Auditor: admin-surface auditor. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` (working tree).
Method: read `D5-final-gaps.md` item 10, `E1-incident-response.md` F1/F2/F12, `C3-base-rate.md` F7,
`A8-ops-realism.md` F10/F11/F12, `E5-fresh-eyes.md`; then **read every admin-bearing file** listed in §0.
Read-only: no repo writes, no DB writes, no external system contacted. Every `file:line` below was read.

**One-line verdict.** The admin surface is _narrow_ (few endpoints, correct self/equal/higher-role guards,
no mass-assignment hole) but it is _blind_ (no audit writer anywhere), _irreversible_ (every destructive
action is a hard delete with a live CASCADE chain behind it), _unmetered_ on the operator nests, and it can
mint peers. A disputed decision 30 days later is unanswerable; a mis-click on `admin_delete` is unanswerable
forever. The single most valuable fix is the audit writer the table was built for and never got.

---

## 0. Ground truth — what I read

| File                                                                                                                                          | What it establishes                                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `src/router.rs:110-158`                                                                                                                       | the nests, their rate-limit layers, which have none                                                                          |
| `src/main.rs:827-858`                                                                                                                         | the private-router layer stack: origin + CSRF + session + route-blocker + no-store; **no global auth, no global limiter**    |
| `src/middlewares/auth_guard.rs:17-22,72-89`                                                                                                   | `ROLE_USER=0 … ROLE_SUPER_ADMIN=4`; `verified_with_role::<N>` = authenticated + verified + not-banned + `role_min(N)` (`>=`) |
| `src/middlewares/static_csrf.rs:56-142`                                                                                                       | every private POST needs a session + `csrf-token`; webhook paths + `/csrf/v1/generate` exempt; GET/HEAD/OPTIONS exempt       |
| `src/middlewares/route_blocker.rs:55-120`                                                                                                     | block matches the **MatchedPath** exactly, applies to the whole private router **before auth**, `/healthz` exempt            |
| `src/services/acl_service.rs` + `admin_acl_v1/*`                                                                                              | `app_constants` CRUD at SuperAdmin; masking is client-declared; no runtime reader                                            |
| `src/modules/user_v1/*`                                                                                                                       | list/view/create/update/delete/change_password at ROLE_ADMIN + inline rank checks                                            |
| `src/modules/post_comment_v1/*`                                                                                                               | hide/unhide/delete/flags_clear/flags_list/summary/details at ROLE_ADMIN                                                      |
| `src/modules/post_v1/*`                                                                                                                       | delete at ROLE_AUTHOR+ownership; series at inline `is_admin()`; `query` scoped by role                                       |
| `src/modules/media_v1/controller.rs`                                                                                                          | delete + moderation; `is_moderator()` bypasses ownership                                                                     |
| `src/modules/analytics_v1/*`                                                                                                                  | nine aggregate read endpoints at ROLE_ADMIN, no limiter                                                                      |
| `src/modules/admin_route_v1/*`, `admin_bans_v1/*`                                                                                             | route block/unblock/delete/sync; ban list/lift/export                                                                        |
| `src/modules/mail_v1/*`, `newsletter_v1/*`, `email_verification_v1/*`                                                                         | suppression CRUD; newsletter send; admin issue_code                                                                          |
| `src/modules/billing_v1/mod.rs:13-53`, `notification_v1/*`, `category_v1/*`, `tag_v1/*`                                                       | billing admin CRUD (dead tables); notification push; category/tag admin CRUD                                                 |
| `src/modules/seed_v1/*`, `src/services/seed/*`, `src/tui/*`, `scripts/{seed_now,ruxlog_tui}.rs`, `Cargo.toml`, `.github/workflows/images.yml` | the seed/TUI system and its build gates                                                                                      |
| `migration/src/m000001_init.rs` + `m000005` + `m000007` + `m000010`                                                                           | the complete FK/CASCADE graph                                                                                                |
| `src/db/sea_connect.rs:32-56` + sqlx-sqlite 0.8.6 `options/mod.rs:185`, `options/connect.rs:39`                                               | `foreign_keys=ON` **per pooled connection** (sqlx re-applies it on every connect) → the CASCADEs below are live              |
| `src/db/sea_models/audit_log/{model.rs,mod.rs}`                                                                                               | the table + model, and nothing else                                                                                          |

Verified corrections to the prior reports, stated up front:

1. **E1 F1 is confirmed** — deleting a comment cascades `comment_flags` (`m000001_init.rs:298-308`,
   `post_comment/actions.rs:384-387`). FKs are enforced per connection by sqlx, so the cascade is real
   today, not theoretical.
2. **`ROLE_MODERATOR` gates no route, but it is not dead.** No `route_layer` uses it, yet inline
   `is_moderator()` checks give a Moderator real powers (§3.5).
3. **D5 item 10's unmetered list is incomplete** — `/analytics/v1`, `/admin/route/v1`, `/admin/acl/v1`,
   `/admin/bans`, `/email_verification/v1`, `/forgot_password/v1` also have no limiter (§6).
4. **D5 item 5's "undo deletes ID ranges" is confirmed and worse than stated**: `seed_all` also creates
   **Admin accounts whose password equals their email** (§5.3, F1).

---

## 1. Capability matrix

Legend — **Role** = minimum `role_min` (0 user / 1 author / 2 moderator / 3 admin / 4 super-admin);
`+own` = additionally scoped to the caller's own rows by an inline check; **RL** = rate limit on the nest
(`/user/v1` 60/min, `/post/comment/v1` 100/min, `/post/v1` 200/min, `/media/v1` 30/min, `/billing/v1`
60/min, `/newsletter/v1` 100/min, `/notification/v1` 100/min — all per-IP **per matched route**;
**none** = no limiter at all); **Audit** = an `audit_logs` row exists (none do — verified §4).

### 1.1 Users (`user_v1`, nest `/user/v1` 60/min)

| #   | Action          | Endpoint                                   | Role               | RL  | Audit | Reversible | Blast radius                                                                                                    |
| --- | --------------- | ------------------------------------------ | ------------------ | --- | ----- | ---------- | --------------------------------------------------------------------------------------------------------------- |
| 1   | List users      | `POST /user/v1/admin/list`                 | 3                  | 60  | no    | n/a        | bulk PII: every email, role, verified flag                                                                      |
| 2   | View user       | `POST /user/v1/admin/view/{id}`            | 3                  | 60  | no    | n/a        | one user's PII                                                                                                  |
| 3   | Create user     | `POST /user/v1/admin/create`               | 3                  | 60  | no    | yes        | any role **≤ caller's** (Admin can mint Admins)                                                                 |
| 4   | Update user     | `POST /user/v1/admin/update/{id}`          | 3, target < caller | 60  | no    | partly     | name/email/role/is_verified/avatar/password; **rotates target `session_auth_secret`** → all target sessions die |
| 5   | **Delete user** | `POST /user/v1/admin/delete/{id}`          | 3, target < caller | 60  | no    | **no**     | hard delete + full CASCADE (§2.1)                                                                               |
| 6   | Change password | `POST /user/v1/admin/change_password/{id}` | 3, target < caller | 60  | no    | no         | silent account takeover; target's sessions revoked                                                              |

### 1.2 Comments (`post_comment_v1`, nest 100/min)

| #   | Action                         | Endpoint                      | Role | Audit | Reversible | Blast radius                                                    |
| --- | ------------------------------ | ----------------------------- | ---- | ----- | ---------- | --------------------------------------------------------------- |
| 7   | List comments                  | `/post/comment/v1/admin/list` | 3    | no    | n/a        | all comments incl. hidden                                       |
| 8   | Hide                           | `/admin/hide/{id}`            | 3    | no    | **yes**    | `hidden=1`                                                      |
| 9   | Unhide                         | `/admin/unhide/{id}`          | 3    | no    | yes        | `hidden=0`                                                      |
| 10  | **Delete**                     | `/admin/delete/{id}`          | 3    | no    | **no**     | row gone **+ `comment_flags` CASCADE** (the reports)            |
| 11  | **Clear flags**                | `/admin/flags/clear/{id}`     | 3    | no    | **no**     | `flags_count=0`; rows survive but the queue signal is destroyed |
| 12  | Flags list / summary / details | `/admin/flags/*`              | 3    | no    | n/a        | reporter identity + free-text reason                            |

### 1.3 Posts (`post_v1`, nest 200/min)

| #   | Action                                 | Endpoint               | Role                  | Audit | Reversible | Blast radius                                                                                                                                                              |
| --- | -------------------------------------- | ---------------------- | --------------------- | ----- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 13  | **Delete post**                        | `/post/v1/delete/{id}` | 1 +own, or 3          | no    | **no**     | CASCADE: `post_revisions`, `scheduled_posts`, `post_views`, `post_access`, `post_likes`, `post_comments` → their `comment_flags`. `media_usage` rows orphan (no cleanup). |
| 14  | Revision list/restore                  | `/revisions/{id}/*`    | 1 +own, or 3          | no    | yes        | content rollback                                                                                                                                                          |
| 15  | Series create/update/delete/add/remove | `/series/*`            | 3 (inline `is_admin`) | no    | partly     | series delete CASCADEs `post_series_posts` links                                                                                                                          |
| 16  | Query posts                            | `/post/v1/query`       | 1 (own) / 2–3 (all)   | no    | n/a        | Moderator sees drafts + bypasses paywall                                                                                                                                  |

### 1.4 Media (`media_v1`, nest 30/min)

| #   | Action              | Endpoint                                    | Role                   | Audit | Reversible | Blast radius                                                                                                                                                                                   |
| --- | ------------------- | ------------------------------------------- | ---------------------- | ----- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 17  | **Delete media**    | `/media/v1/delete/{id}`                     | 1 (own) **or 2** (any) | no    | **no**     | R2 object deleted, row deleted, `media_variants` CASCADE, `posts.featured_image_id` → NULL, `categories.cover/logo` → NULL. **No usage check**: media in a live published post can be deleted. |
| 18  | View / list / usage | `/media/v1/{view,list/query,usage/details}` | 1 (own) or 2 (all)     | no    | n/a        | Moderator reads any user's media **and the owner's email** (`controller.rs:763`)                                                                                                               |
| 19  | Create              | `/media/v1/create`                          | 1                      | no    | —          | moderation is **absent** when `IMAGE_MODERATION_ENABLED` is false (E1 F5)                                                                                                                      |

### 1.5 Analytics (`analytics_v1`, **no limiter**)

| #   | Action            | Endpoint          | Role | Audit | Notes                                                                                                                                                                                                                                                                                                            |
| --- | ----------------- | ----------------- | ---- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 20  | 9 aggregate reads | `/analytics/v1/*` | 3    | no    | counts only (registration, verification, publishing, page-views, comment-rate, newsletter, media, dashboard). `page_views` reads raw `ip_address` to count uniques but returns only aggregates → no PII in the response. Unmetered = an admin session can hammer `FULL OUTER JOIN`s on the pool-of-1 (self-DoS). |

### 1.6 Route blocker (`admin_route_v1`, **no limiter**)

| #   | Action                                    | Endpoint                         | Role | Audit | Reversible    | Blast radius                                                                                                                                  |
| --- | ----------------------------------------- | -------------------------------- | ---- | ----- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 21  | **Block route**                           | `/admin/route/v1/block`          | 3    | no    | yes (unblock) | global 503 for that matched pattern, all users, **before auth**; pattern is any 1–255-char string; can block `/admin/route/v1/unblock` itself |
| 22  | Unblock / update / delete                 | `/unblock`, `/update`, `/delete` | 3    | no    | —             | route-status row removed (the record of the block is deleted)                                                                                 |
| 23  | Sync / sync_interval pause/resume/restart | `/sync*`                         | 3    | no    | yes           | runtime loop state (blocks still apply immediately on mutation)                                                                               |

### 1.7 ACL constants (`admin_acl_v1`, **no limiter**)

| #   | Action                                 | Endpoint               | Role  | Audit | Notes                                                                                                                                                                                                                                     |
| --- | -------------------------------------- | ---------------------- | ----- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 24  | List/get/create/update/delete constant | `/admin/acl/v1/*`      | **4** | no    | `app_constants` has **no runtime reader** outside the ACL service (`rg 'AppConstant::' src/` → acl_service only) → write-only config surface + unaudited secret store. Masking is client-declared (`is_sensitive` from the request body). |
| 25  | Sync / import_env                      | `/sync`, `/import_env` | 4     | no    | copies env vars into the DB at rest, filtered by a **name heuristic** (`acl_service.rs:76-138`)                                                                                                                                           |

### 1.8 Bans (`admin_bans_v1`, merged, **no limiter**)

| #   | Action         | Endpoint                 | Role                               | Audit | Reversible | Blast radius                                                                                                               |
| --- | -------------- | ------------------------ | ---------------------------------- | ----- | ---------- | -------------------------------------------------------------------------------------------------------------------------- |
| 26  | List bans      | `GET /admin/bans`        | 3                                  | no    | n/a        | every banned CIDR + expiry                                                                                                 |
| 27  | **Lift ban**   | `DELETE /admin/bans`     | 3 (+CSRF)                          | no    | yes        | removes `quran-ban:{unit}` + `ratelimit:{unit}:quran-v1` from L1+L2 → an IP ban disappears with no record of who lifted it |
| 28  | Export bans    | `GET /admin/bans/export` | 3 **or** `BAN_EXPORT_TOKEN` bearer | no    | n/a        | a second credential that bypasses session+CSRF for reads (`controller.rs:188-241`); read-only by construction              |
| —   | **Create ban** | —                        | —                                  | —     | —          | **does not exist.** `user_bans` has no writer (E1 F8); the only automatic ban engine is default-OFF                        |

### 1.9 Mail / newsletter / verification (operator mail)

| #   | Action                          | Endpoint                                       | Role   | RL       | Audit | Blast radius                                                                                                                                                              |
| --- | ------------------------------- | ---------------------------------------------- | ------ | -------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 29  | List suppressions               | `GET /mail/v1/suppression`                     | 3      | none     | no    | every suppressed address + diagnostic                                                                                                                                     |
| 30  | **Add suppression**             | `POST /mail/v1/suppression`                    | 3      | none     | no    | **per-address auth kill switch**: a suppressed address silently drops verification + password-reset mail (`router.rs:251-259` logs `debug!` and returns `Ok`)             |
| 31  | Remove suppression              | `DELETE /mail/v1/suppression`                  | 3      | none     | no    | un-suppresses a bounced/complained address → reputation damage                                                                                                            |
| 32  | Webhook                         | `POST /mail/v1/webhook/{provider}`             | public | none     | no    | HMAC+timestamp+replay verified; performs an HMAC per request on the single connection (D5 1c)                                                                             |
| 33  | **Newsletter send**             | `POST /newsletter/v1/send`                     | 3      | 100      | no    | mails **every confirmed subscriber**; subject/body free-form; background task; no cap, no cancel, no confirm                                                              |
| 34  | Subscriber list                 | `POST /newsletter/v1/subscribers/list`         | 3      | 100      | no    | subscriber emails                                                                                                                                                         |
| 35  | **Issue verification code**     | `POST /email_verification/v1/admin/issue_code` | 3      | **none** | no    | emails a code to any user's address, ≤29/h per recipient and uncapped across recipients (G3 §6.1) → mail-bomb any user; no rank check (can target a SuperAdmin's address) |
| 36  | List / delete verification rows | `/admin/list`, `/admin/delete`                 | 3      | none     | no    | delete = the target must re-request                                                                                                                                       |

### 1.10 Content config (category / tag / billing / notification)

| #   | Action                                                                                    | Endpoint                        | Role       | RL       | Audit | Blast radius                                                                                                                                                                                                           |
| --- | ----------------------------------------------------------------------------------------- | ------------------------------- | ---------- | -------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 37  | **Delete category**                                                                       | `POST /category/v1/delete/{id}` | 3          | **none** | no    | **`posts.category_id` is ON DELETE CASCADE** → every post in the category, with its revisions, views, likes, comments and comment flags. One request, no confirmation, no record. (NEW — no prior report covers this.) |
| 38  | Create/update category, list/query                                                        | `/category/v1/*`                | 3 (writes) | none     | no    | public list/view unauthenticated                                                                                                                                                                                       |
| 39  | Create/update/delete tag, view/query                                                      | `/tag/v1/*`                     | 3 (writes) | none     | no    | `posts.tag_ids` is JSON, no FK → stale ids survive a tag delete                                                                                                                                                        |
| 40  | Plan CRUD, subscription cancel, payment/invoice/discount list, discount CRUD, post access | `/billing/v1/*`                 | 3          | 60       | no    | **dead module**: eight of its nine tables have no migration (E3) → 500s; latent the day the tables land                                                                                                                |
| 41  | **Create notification**                                                                   | `POST /notification/v1/create`  | 3          | 100      | no    | writes a row + up to 20 FCM pushes to any user; no per-recipient cap (D5 item 3)                                                                                                                                       |

### 1.11 Seed system (feature-gated `seed-system`, **not in the shipped image**)

| #   | Action                         | Endpoint / surface                                | Role                                      | Audit | Blast radius                                                                                                                           |
| --- | ------------------------------ | ------------------------------------------------- | ----------------------------------------- | ----- | -------------------------------------------------------------------------------------------------------------------------------------- |
| 42  | 19 seed endpoints + `seed_all` | `/admin/seed/v1/*`                                | 4 **and** `#[cfg(feature="seed-system")]` | no    | fabricates 50 users (10% Admin, password = email), posts, comments, sessions with fake IPs, media rows, flags, subscribers, route rows |
| 43  | `seed_now` binary              | `cargo run --bin seed_now --features seed-system` | none (DB access is the auth)              | no    | same, from a shell, **no production guard**, reads `DATABASE_URL`                                                                      |
| 44  | TUI                            | `ruxlog_tui` binary                               | none                                      | no    | seed any subset, read all users' emails, **undo a seed run on one keypress**                                                           |
| 45  | Undo                           | TUI `u`/Enter                                     | none                                      | no    | `delete_many` by **ID range** over 19 tables **including `users`** (`seed/undo.rs:29-105`, `types.rs:71-80`)                           |

**Verified: the seed system is NOT in the shipped image.** `images.yml:187-192` builds
`cargo build --release --locked -p ruxlog --features vendored-openssl`; `seed-system` is not in `default`
(`Cargo.toml:11-13`), the `ruxlog_tui`/`seed_now` bins are `required-features = ["seed-system"]`
(`Cargo.toml:192-200`), `tui` is `#[cfg(feature = "seed-system")]` (`lib.rs:17-18`), the seed routes are
`#[cfg(feature = "seed-system")]` (`router.rs:155-158`), and `main.rs` never references the TUI. The risk is
purely the dev binary being pointed at a production `DATABASE_URL` (F1).

---

## 2. The destruction audit

### 2.1 What `POST /user/v1/admin/delete/{id}` destroys

`user_v1/controller.rs:269-318` → `user/actions.rs:486-497` (`delete_by_id`, after deleting the user's
`media_usage` rows). With FKs ON, the CASCADE chain from `m000001_init.rs`, `m000005`, `m000007`,
`m000010` is:

| Table                                               | FK                                              | Effect                                                                                                   |
| --------------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `user_sessions`                                     | `user_id` CASCADE (`:55-56`)                    | all session rows gone — raw IP + UA evidence for the account                                             |
| `user_bans`                                         | `user_id` CASCADE (`:67-68`)                    | **the ban record is gone** — an appeal or LE request has nothing to review                               |
| `posts`                                             | `author_id` CASCADE (`:119-120`)                | **the author's entire body of work**                                                                     |
| `post_revisions`                                    | `post_id` CASCADE (`:132-133`)                  | all revision history of those posts                                                                      |
| `scheduled_posts`                                   | `post_id` CASCADE (`:142-143`)                  | pending publications                                                                                     |
| `post_series_posts`                                 | `post_id` CASCADE (`:162-163`)                  | series membership of those posts                                                                         |
| `post_views`                                        | `post_id` CASCADE (`:172-173`)                  | view rows, incl. raw IP/UA evidence                                                                      |
| `post_access`                                       | `post_id` CASCADE (`:182-183`)                  | paywall config for those posts                                                                           |
| `post_likes`                                        | `post_id`/`user_id` CASCADE (`:190-193`)        | likes on those posts **and** the user's likes on everyone else's posts                                   |
| `post_comments`                                     | `post_id`/`user_id` CASCADE (`:293-296`)        | comments on those posts **and** the user's comments elsewhere                                            |
| `comment_flags`                                     | `comment_id`/`user_id` CASCADE (`:304-307`)     | **every report attached to those comments, plus every report the user filed on other people's comments** |
| `forgot_passwords` / `email_verifications`          | `user_id` CASCADE (`:271-272`, `:280-281`)      | reset/verification state                                                                                 |
| `devices`, `notifications`, `user_oauth_identities` | `user_id` CASCADE (`m000005:21-22,37-38,53-54`) | FCM tokens, notification history, OAuth links                                                            |
| `bookmarks`, `bookmark_folders`                     | `user_id` CASCADE (`m000007:19-20,34-35`)       | all saved verses                                                                                         |
| `passkey_credentials`                               | `user_id` CASCADE (`m000010:25-26`)             | credentials                                                                                              |
| `media`                                             | `uploader_id` **SET NULL** (`:45-46`)           | rows survive, **orphaned**; the R2 objects are never reclaimed (no GC)                                   |
| `audit_logs`                                        | `user_id` **SET NULL** (`:262-263`)             | any pre-existing audit row survives but loses its actor id                                               |
| `media_usage` (user rows)                           | explicit delete (`actions.rs:489-490`)          | usage rows removed; `media` rows kept                                                                    |

**Evidence that survives: nothing.** No snapshot, no soft delete, no `deleted_at`, no `audit_logs` row, no
notice to the target, no notification to anyone. The stdout `info!(user_id, "Admin deleted user")` line
(`controller.rs:296`) is the entire record, and it is rotation-dependent and unqueryable. The subject's
appeal is impossible _because the ban row that would be reviewed was deleted with the account_
(`m000001_init.rs:67-68`) — E1 F2, confirmed and sharpened: the CASCADE reaches `user_bans` itself.

Also: `admin_delete` blocks self and equal/higher targets (`:279-292`), so an Admin cannot delete a peer
Admin or the SuperAdmin. But a SuperAdmin can delete any Admin, and any Admin can delete any
Moderator/Author/User. There is no second pair of eyes anywhere in the flow.

### 2.2 What `POST /post/comment/v1/admin/delete/{id}` destroys — E1 F1 verified

`post_comment/actions.rs:384-387` = `delete_by_id` → `comment_flags.comment_id` FK is
`ON DELETE CASCADE` (`m000001_init.rs:304-305`) → **deleting a comment deletes the reports on it**. FK
enforcement is live per connection (sqlx-sqlite 0.8.6 `options/mod.rs:185` + `options/connect.rs:39` sets
`PRAGMA foreign_keys = ON` on every pooled connect; the app's own pragma at `sea_connect.rs:50` is
redundant but harmless). The flag's `user_id` FK is also CASCADE, so deleting a _flagger's_ account deletes
their reports on other people's comments.

Two aggravating details:

- `admin_flags_clear` (`post_comment/actions.rs:389-400`) zeroes `flags_count` while the rows survive — the
  queue's _count_ signal is destroyed even when the evidence is not, and the route returns `Ok` for a
  nonexistent comment (`controller.rs:325-331`).
- `comment_flag::Entity::clear_flags` (`comment_flag/actions.rs:179-186`) is a `delete_many` with **zero
  callers** — a loaded gun for the next developer who wants "clear the flags".

### 2.3 What `POST /category/v1/delete/{id}` destroys (NEW)

`posts.category_id` is `ON DELETE CASCADE` (`m000001_init.rs:121-122`). Deleting a category deletes **every
post in it**, and each post's delete cascades `post_revisions`, `scheduled_posts`, `post_views`,
`post_access`, `post_likes`, `post_comments`, and each comment's `comment_flags`. One admin POST with a
category id — no confirmation, no count shown, no `audit_logs` row, no dry-run. `post_v1::delete` has the
same shape for a single post (`controller.rs:289-311`).

### 2.4 Media delete

`media_v1/controller.rs:804-860`: `can_delete_media` allows the owner, **or any moderator/admin for any
media** (`:543-548`), and the route's minimum is ROLE_AUTHOR so a Moderator passes. The R2 object is
deleted first (`:832-849`), then the row (`:851`). `media_variants` CASCADE; `posts.featured_image_id` and
`categories.cover_id/logo_id` go NULL. There is **no usage check**: media referenced by a published post can
be deleted, leaving the post live with a dead image URL that was already indexed/shared. Irreversible,
unlogged.

### 2.5 Flag clear / ban lift / route delete

- `admin_flags_clear`: destroys the count, keeps rows, returns 200 for a missing comment. Reversible only by
  recomputing from `comment_flags` (nothing does).
- `admin_bans_v1` DELETE: removes the ban from L1+L2 (`controller.rs:141-153`) — an IP ban vanishes. The
  `quran-ban:` row _was_ the only record; there is no `audit_logs` row, so "who lifted this ban and why" is
  unanswerable.
- `admin_route_v1` delete: removes the `route_status` row (`route_blocker_service.rs:165-180`) — the record
  of _why_ a route was blocked is deleted with it.

### 2.6 Seed undo

`seed/undo.rs:29-105` iterates 19 tables and issues `delete_many` filtered `id >= from AND id <= to`, where
`from = before_max + 1` and `to = after_max` (`types.rs:71-80`). Any row created **between** the seed's first
and last insert — by a real user, a real comment, a real session — is inside the range and is deleted.
`users` is in the list (`:102`), so real accounts inside the range are deleted with full CASCADE. Today the
undo is dead only because `seed_runs` has no migration and the insert error is swallowed
(`base.rs:614-627`); C3 F6's own fix (add `seed_runs`) **arms** it.

---

## 3. The ACL audit

### 3.1 How a role is assigned

`POST /user/v1/admin/update/{id}` with `role: "moderator"|"author"|"admin"|"super-admin"|"user"`
(`validator.rs:92-93`, `enums/user.rs:37-46`). Guards (`controller.rs:328-358`):

- requested role `<= caller_level` — so an Admin may assign Admin or anything below, **never** SuperAdmin;
- target's current role must be **strictly lower** than the caller — so no Admin can edit a peer Admin or the
  SuperAdmin, and nobody can edit themselves (self-edit is blocked because `target.role >= caller_level`).

### 3.2 Can an Admin escalate? Can it mint peers?

It cannot exceed its own level. It **can** mint unlimited peers: `admin_create` accepts `role <= caller_level`
(`controller.rs:249-259`) and `admin_update` accepts `role <= caller_level` on a lower target
(`:333-346`). So one compromised Admin account becomes _n_ Admin accounts, each able to do everything in §1,
with no notification to the SuperAdmin and no audit row. This is privilege _widening_ rather than vertical
escalation, and it is the most likely persistence path for a compromised admin session: mint a second Admin
before the first is noticed.

### 3.3 Demote / ban another admin?

- **Demote**: blocked for equal/higher targets (`:348-358`). A SuperAdmin can demote an Admin.
- **Delete**: blocked for equal/higher (`:283-292`).
- **Reset password**: blocked for equal/higher (`:397-409`).
- **Ban**: impossible — there is no ban-creation route at all (E1 F8, `admin_bans_v1` is lift-only). So a
  rogue Admin **cannot** be banned through the product; the only tool is delete (SuperAdmin) or the
  SuperAdmin rotating the rogue admin's password.
- **Session revocation**: the only "kick" primitive is `admin_change_password` (rotates
  `session_auth_secret`, `user/actions.rs:223-228`) or the target's own `sessions/terminate` route. There is
  no admin-side session revoke for a lower-ranked account, and no way to revoke a _peer_ admin's sessions.

### 3.4 Separation of duties

None in code. One person (the operator) is SuperAdmin; every destructive endpoint accepts a single request
from that one session. `ROLE_MODERATOR` is the only second seat the schema offers and no route uses it.

### 3.5 `ROLE_MODERATOR` — assignable, and NOT dead

Assignable: yes, by any Admin (§3.1). No `route_layer` uses it (verified: grep for `ROLE_MODERATOR` in
`src/modules/**` returns only `auth_guard.rs` and the `ruxlog-types` enum). **But inline `is_moderator()`
checks give it live powers**, because `is_moderator()` is `role >= 2` (`user/model.rs:49-51`) and several
authorization decisions are inline rather than in a route gate:

| Capability                               | Code                                                                           | What it means                                                          |
| ---------------------------------------- | ------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Read **any** user's media                | `media_v1/controller.rs:539-541` (`can_view_media`)                            | plus the owner's **email** in usage details (`:763`)                   |
| Delete **any** user's media              | `media_v1/controller.rs:543-548`                                               | irreversible R2 delete; route min is ROLE_AUTHOR so a Moderator passes |
| Bypass the paywall on **any** post       | `post_v1/controller.rs:35-40` (`viewer_bypasses_paywall`), `:121` (`is_staff`) | reads subscriber-only/paid content                                     |
| See comments on hidden/unpublished posts | `post_comment_v1/controller.rs:44-46`                                          | draft surveillance                                                     |
| Attach any user's media to a post        | `post_v1/controller.rs:58`                                                     | content misuse                                                         |
| See all posts incl. drafts               | `post_v1/controller.rs:407-418` (Moderator arm is a no-op → no author filter)  |                                                                        |

So the correct statement is: **`ROLE_MODERATOR` is a live privilege tier that no route _gate_ uses.** It is
still the right seat for E1's triage queue, but promoting anyone to Moderator today silently grants
"delete any media + read any email + read all paid content". That is an escalation step, not a neutral
label, and it is exactly what E1 F12's fix would hand a second human.

---

## 4. The audit-trail audit

### 4.1 Every admin action with no record

`audit_logs` exists (`m000001_init.rs:252-264`), has the right columns and a `SET NULL` FK to `users`
(`:262-263`), has a model (`audit_log/model.rs`) — and **zero writers**. Verified: `rg 'audit_log|AuditLog|write_audit' src/`
returns only the model and its `mod.rs` registration. No `actions.rs`, no `slice.rs`, no service.

The unrecorded mutation set (from §1; the sites D5/C3 named plus the ones they missed):

| #   | Action                                         | Site                                          | What is lost                                      |
| --- | ---------------------------------------------- | --------------------------------------------- | ------------------------------------------------- |
| 1   | user delete                                    | `user_v1/controller.rs:294`                   | **everything** (§2.1)                             |
| 2   | user update (role/email/verified/password)     | `:361`                                        | who changed a role, when                          |
| 3   | user create                                    | `:262`                                        | who minted an Admin                               |
| 4   | admin password reset                           | `:411`                                        | the takeover itself                               |
| 5   | comment hide / unhide                          | `post_comment_v1/controller.rs:240,268`       | DSA Art 17 statement-of-reasons                   |
| 6   | comment delete                                 | `:296`                                        | content + reports (§2.2)                          |
| 7   | flag clear                                     | `:324`                                        | the count signal                                  |
| 8   | post delete                                    | `post_v1/controller.rs:299`                   | content + comments + flags                        |
| 9   | media delete                                   | `media_v1/controller.rs:851`                  | object key + hash (the only pointer to the bytes) |
| 10  | route block / unblock / update / delete        | `admin_route_v1/controller.rs:30,59,84,121`   | the availability decision                         |
| 11  | ban lift                                       | `admin_bans_v1/controller.rs:141-153`         | who un-banned which CIDR                          |
| 12  | ACL constant create/update/delete + import_env | `admin_acl_v1/controller.rs:129,154,168,24`   | config/secret changes                             |
| 13  | newsletter send                                | `newsletter_v1/controller.rs:233`             | what was mailed to how many people                |
| 14  | suppression add / remove                       | `mail_v1/controller.rs:48,75`                 | who cut off whose mail                            |
| 15  | verification issue / delete                    | `email_verification_v1/controller.rs:264,230` | mail sent to a user                               |
| 16  | category create/update/**delete**              | `category_v1/controller.rs`                   | the post-cascade (§2.3)                           |
| 17  | tag create/update/delete                       | `tag_v1/controller.rs`                        | stale `tag_ids`                                   |
| 18  | notification create                            | `notification_v1/controller.rs:92`            | push sent to a user                               |
| 19  | billing admin mutations                        | `billing_v1/controller.rs`                    | dead today; latent                                |
| 20  | `seed_all` / seed endpoints / TUI undo         | `seed/base.rs:171-203`, `tui/app.rs:613-634`  | fabrication + range deletion                      |

Reads with no record (see §7): `admin_list`, `admin_view`, `admin_flags_details`, `media view/list/usage`,
`ban export`, `subscriber list`, `suppression list`.

### 4.2 What a disputed decision looks like 30 days later

**Nothing.** Concretely:

- _"Why was my account deleted?"_ — the account, its ban row, its sessions, its comments and the flags
  against them are gone. The only trace is an `info!` line in a container log that Dokploy rotates.
- _"Who reported my comment, and what did they say?"_ — if the comment was deleted, the flags are gone
  (`comment_flags` CASCADE). If the flags were cleared, `flags_count` is 0 and a cleared flag is
  indistinguishable from a never-seen one (E1 F4).
- _"Did anyone look at my data?"_ — no read is logged; `user_sessions.device/ip_address` were readable by the
  owner only, but `admin_view`/`media view` return PII with no trace.
- _"Who lifted my IP ban?"_ — nothing; the ban row was the record.
- _"Why is this route 503?"_ — if the route-status row was deleted, the reason is gone.
- _"Was this post hidden or deleted, and by whom?"_ — `hidden` state is at least in the row; a delete is not.
- A DSA Art 17 / GDPR Art 15 answer is impossible for any of the above: the controller cannot demonstrate
  that a decision was lawful, consistent, or even made by a human rather than the operator's cat.

### 4.3 The minimum audit writer (design)

The table needs no migration. `~150 LOC` total.

```rust
// src/services/audit.rs
pub struct AuditEvent {
    pub action: &'static str,          // "user.delete", "comment.hide", "route.block", "read.user"
    pub resource_type: &'static str,   // "user", "comment", "media", "route", "ban", "constant", "mail"
    pub resource_id: String,
    pub metadata: serde_json::Value,   // before/after, counts, reason; NEVER secrets/passwords
    pub ip_address: Option<String>,    // from ClientIp when the handler has it
    pub user_agent: Option<String>,
}

/// Fire-and-forget is WRONG for destructive actions: the row must be written in the
/// same transaction as the mutation, or the audit can be missing exactly when it matters.
pub async fn record<C: ConnectionTrait>(
    conn: &C, actor: Option<i32>, ev: AuditEvent,
) -> Result<(), ErrorResponse>;
```

Rules that make it survive scrutiny:

1. **Same transaction as the mutation** for every destructive action (pass the `txn`, not `sea_db`). For
   reads, best-effort `sea_db` is acceptable.
2. `metadata` always carries `actor_id` **inside the JSON** as well as in `user_id`, because the FK is
   `ON DELETE SET NULL` (`m000001_init.rs:262-263`) — a later user delete would otherwise erase the actor of
   every action they ever took.
3. Never write a password, token, code hash, or secret value. ACL events record **key names only**.
4. Destructive events record a **pre-state snapshot** (`email`, `role`, `flags_count`, `object_key`,
   `content_hash`, row counts) — that is what makes the deletion reviewable after the fact.
5. `created_at` is the table's `CURRENT_TIMESTAMP` default; do not add a second timestamp convention.

Call sites the writer must cover (each is a one-line call; the transaction is already open in most of them):
`user_v1::{admin_create, admin_update, admin_delete, admin_change_password}`;
`post_comment_v1::{admin_hide, admin_unhide, admin_delete, admin_flags_clear}`;
`post_v1::delete`; `media_v1::delete`;
`admin_route_v1::{block_route, unblock_route, update_route_status, delete_route}`;
`admin_bans_v1::delete_ban`;
`admin_acl_v1::{create_constant, update_constant, delete_constant, import_env_constants}`;
`newsletter_v1::send`; `mail_v1::{create_suppression, delete_suppression}`;
`email_verification_v1::{admin_issue_code, admin_delete}`;
`category_v1::{create, update, delete}`; `tag_v1::{create, update, delete}`;
`notification_v1::admin_create`;
`seed_v1::seed` + `seed::undo_seed_run` (action `seed.run` / `seed.undo` with the range JSON).
Reads (action `read.*`, best-effort): `user_v1::{admin_list, admin_view}`,
`post_comment_v1::admin_flags_details`, `media_v1::{view, find_with_query, list_usage_details}`,
`admin_bans_v1::export_bans`, `newsletter_v1::list_subscribers`, `mail_v1::list_suppressions`.

**Cheap ordering win:** wire only the destructive set first (rows 1–16 above). That is ~10 call sites and it
converts "no record" into "a queryable trail" for everything that cannot be undone.

---

## 5. The seed / TUI audit

### 5.1 Is the TUI in the shipped image?

**No.** Verified in §1.11: `images.yml:187-192` builds `-p ruxlog --features vendored-openssl`; `seed-system`
is off by default (`Cargo.toml:11-13`); the two bins are `required-features = ["seed-system"]`
(`Cargo.toml:192-200`); the `tui` module is feature-gated (`lib.rs:17-18`); the seed routes are
feature-gated (`router.rs:155-158`); `docker-compose.yml` invokes no seeder; the `justfile` has no seed/tui
recipe. So a production deploy cannot serve `/admin/seed/v1/*` and cannot run the TUI.

### 5.2 What it can do if it is built

- **Seed everything**: 50 users, 10 categories, 50 tags, posts, comments, revisions, series, scheduled
  posts, media rows, media variants, media usage, comment flags, 100 newsletter subscribers, route rows,
  email-verification and forgot-password code rows, `post_views` with **fake raw IP + UA** rows.
- **Seed a subset** with a size preset and a static seed (reproducible fabrication).
- **Read every user's email** (Users screen: `fetch_users` → `user::Entity::find()`, `app.rs:959-965`).
- **Undo a seed run on one keypress** (`app.rs:613-634`: `u` or `Enter` on the confirm screen).

### 5.3 What happens if it runs against production

1. **Admin backdoor.** `seed/base.rs:171-188` creates 50 users with `password = user.email` and
   `role = Admin for ~10%` (then Author ~45%, User rest), `is_verified = true`. `seed/custom.rs:135-155`
   does the same with a flat 1-in-3 Admin chance. Emails come from `FreeEmail()` (`base.rs:32`,
   `custom.rs:143`) — a fixed, public word list. An attacker who knows seeding ran can guess
   `first_last@domain` + password == email, or simply read the addresses off `admin_list` if they get any
   admin session. Either way: **a live, verified, guessable Admin account exists and nothing in the product
   reports that it was created.** `seed_now.rs:5-24` has **no** `is_production()` check, and neither does the
   TUI. `DATABASE_URL` is the only knob (`seed_now.rs:7-12`), and `.env.dev` is the fallback.
2. **Fabricated evidence.** 50 users, hundreds of comments, and `post_views`/`user_sessions` rows carrying
   synthetic raw IPs (`base.rs:655`, `seed_v1/controller.rs:397,695`) are indistinguishable from real
   records (C3 F6). Every abuse counter, ban decision, and LE response computed from them is wrong.
3. **The undo is a data-loss weapon.** If `seed_runs` ever exists (C3's own fix), `undo` deletes by ID range
   across 19 tables including `users` (§2.6). Real rows created during the seed window are inside the range.
4. **No record.** No `audit_logs` row, no provenance marker on any row.

### 5.4 The minimum guard

1. **Prod refusal, in code, at the entry points** — not in docs: `if is_production()? { eprintln!(...); exit(1) }`
   in `seed_now::main`, `ruxlog_tui::main`, **and** at the top of `seed_all_with_progress` +
   `seed_custom` (so the HTTP route is covered too, in case the feature is ever enabled by mistake). One
   helper, four call sites. `is_production()` already exists (`config/settings.rs:30`) and is used this way
   by `forgot_password_v1`, `email_verification_v1`, `turnstile`, `route_blocker`, `client_ip`.
2. **Stop creating Admins with guessable passwords.** Either drop the Admin role from the seeder entirely, or
   generate a random password and print it once to stdout. `password = email` must not survive.
3. **Typed confirmation on undo**: require the operator to type the run id (`prompt_for_u32` already exists
   at `app.rs:981`) and print the exact row counts to be deleted before the delete runs.
4. **Undo by recorded keys, not ranges** (the correct fix, larger): record the inserted primary keys (or a
   `seed_run_id` column on each seeded table) and delete `WHERE seed_run_id = ?`. Until then, the range
   delete is only safe on a DB that has had no writes since the seed.
5. **Provenance marker** (C3 F6's fix, still worth doing): prefix seeded emails with a reserved tag
   (`seed+…@seed.invalid`) and exclude them from every abuse aggregate.

---

## 6. The rate-limit audit

The private router has **no global limiter** (`main.rs:827-858` — origin guard, CSRF, session, CORS,
route-blocker, no-store, security headers only). Every nest must bring its own layer. D5 named four
unmetered nests; the real list is **ten**:

| Nest                     | Limiter  | Auth on the nest                                                     | What an unauthenticated caller achieves                                                                                                                                                                                                              |
| ------------------------ | -------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/feed/v1`               | **none** | public                                                               | `GET /rss`, `/atom`: each request runs a `posts` query + `load_post_access_map` on the **single** SQLite connection. Unbounded → the cheapest public DoS in the app. (Cache-Control 300 s is set, but nothing guarantees an edge cache; cf. E5 #10.) |
| `/category/v1`           | **none** | public reads, ROLE_ADMIN writes                                      | unbounded `find_all`; writes need an admin session                                                                                                                                                                                                   |
| `/tag/v1`                | **none** | public reads, ROLE_ADMIN writes                                      | same                                                                                                                                                                                                                                                 |
| `/mail/v1`               | **none** | public webhook, ROLE_ADMIN admin                                     | `POST /mail/v1/webhook/{provider}`: unauthenticated, CSRF-exempt, performs an HMAC + dedup + a suppression upsert per request on the single connection (D5 1c). No auth bypass — it is a free CPU/DB amplifier.                                      |
| `/email_verification/v1` | **none** | `unverified` auth for `/verify`,`/resend`; ROLE_ADMIN for `/admin/*` | nothing unauthenticated (auth layer), but `/admin/issue_code` is an unmetered mail sender for any admin session (F12)                                                                                                                                |
| `/forgot_password/v1`    | **none** | public                                                               | the account-existence oracle (E5 #5 / D5 1a) at the in-handler abuse-limiter rate only                                                                                                                                                               |
| `/analytics/v1`          | **none** | ROLE_ADMIN                                                           | nothing unauthenticated; an admin session can hammer 9 aggregate `FULL OUTER JOIN` queries on the pool-of-1                                                                                                                                          |
| `/admin/route/v1`        | **none** | ROLE_ADMIN                                                           | nothing unauthenticated; a session can block/unblock at line rate (each mutation writes the DB + re-syncs the cache + refreshes the snapshot — self-DoS)                                                                                             |
| `/admin/acl/v1`          | **none** | ROLE_SUPER_ADMIN                                                     | same; `import_env` walks all env vars and upserts each one                                                                                                                                                                                           |
| `/admin/bans` (merged)   | **none** | ROLE_ADMIN / export token                                            | nothing unauthenticated; `GET` with a leaked `BAN_EXPORT_TOKEN` is unlimited                                                                                                                                                                         |

Two structural notes:

1. The CSRF guard is not a rate limit: an unauthenticated caller can `POST /csrf/v1/generate` (60/min) to
   obtain a session + token and then reach any CSRF-protected route. Origin guard passes requests with no
   `Origin` header (non-browser clients), so curl reaches the private API directly.
2. Adding a nest layer is one line per nest. There is no reason for any of the ten to stay unmetered.

---

## 7. The read-audit question

**An admin can read:** every user's email/role/verification/2FA-enabled/timestamps (`admin_list`,
`admin_view`); every comment incl. hidden ones and every flag with the reporter's identity and free text
(`admin/list`, `flags/*`); every media row and the owner's email (`media/{view,list/query,usage/details}`);
every suppressed address with diagnostics; every subscriber address; every banned CIDR; every verification
row; aggregate analytics. **None of it is logged.** `rg` for any read-side audit write returns zero.

Is any read logged? Only stdout `info!` lines (`"Admin viewed user"`, `"Admin listed comment flags"`), which
are not queryable, not attributable after rotation, and absent from `audit_logs`.

Privacy exposure (A5/B9's requirement): the app stores raw IP + UA in `user_sessions` and `post_views`; the
operator can correlate them with an account via `admin_view` (email) and the flag queue (reporter identity).
That is exactly the "operator read access to device/account data" class A5/B9 say must be logged — and it is
the class with no log. The `post_views` raw-IP writer is still public (E1 F3), so the correlation surface is
live.

**Minimum read-logging design.** Reuse the same writer; do not build a second table.

1. **Class A — bulk PII reads** (`admin_list`, `admin_view`, `newsletter/subscribers/list`,
   `mail/suppression` list, `media list/query` + `usage/details`, `flags/list|summary|details`,
   `ban export`): one `audit_logs` row per request with `action='read.<subject>'`,
   `resource_type`, `resource_id` (the query or the target id), `metadata = {count, filters}`, plus
   `ip_address`/`user_agent` from the request. Best-effort write; never fail the read because the audit
   failed.
2. **Class B — evidence reads** (anything that reveals a _third party's_ identity: flag details, media
   usage with emails): always logged, and the row is the access record a DSAR/dispute needs.
3. Retention: audit rows for reads are `enforcement_12mo` (E1 §5.1); access to them should itself be
   restricted to SuperAdmin + the operator.
4. Do **not** log the values read (no emails, no content) — log the _fact_ and the count. Otherwise the audit
   table becomes a second copy of the PII it is protecting.
5. State the rule in the docs (the abuse plan, then `documentation/history.md`) so a future admin UI cannot add an
   unlogged read path by accident.

---

## 8. Attack

### 8.1 A compromised admin session (or a malicious Admin)

| #   | Move                                                                                                                          | Detection today                                                 |
| --- | ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 1   | Mint a second Admin (`admin_create` role=admin) and keep it after the first is locked down                                    | none — no audit, no notification                                |
| 2   | Delete the account that is investigating them (any Moderator/Author/User) — CASCADE destroys the ban/session/comment evidence | one `info!` line                                                |
| 3   | Delete the comments and posts that carry reports against them — the flags die with the content                                | one `info!` line                                                |
| 4   | Delete the category holding a rival's posts — every post in it, cascaded                                                      | one `info!` line                                                |
| 5   | Clear `flags_count` on every flagged comment (the reports survive as rows but the queue count reads 0)                        | one `info!` line per call                                       |
| 6   | Reset any lower-ranked user's password → silent account takeover; their sessions are revoked so the target may not notice     | none                                                            |
| 7   | Add a victim's address to the mail suppression list → they silently lose password reset and verification                      | none (the send logs `debug!` and returns `Ok`)                  |
| 8   | Newsletter-send arbitrary content to every subscriber                                                                         | none                                                            |
| 9   | Block a route (e.g. `/post/v1/list/published`, or `/admin/route/v1/unblock` itself)                                           | one `info!` line; the row records the pattern but not the actor |
| 10  | Lift every IP ban                                                                                                             | none                                                            |
| 11  | Read every user's email, every flag's reporter, every media owner's email                                                     | none                                                            |
| 12  | Delete every media object in R2 (Moderator can do this too)                                                                   | none                                                            |
| 13  | Store/read arbitrary config values incl. secrets in `app_constants` (SuperAdmin)                                              | none                                                            |

**The detection story is one stdout line per mutation and nothing else.** No 429 counter is visible (429s are
`debug!`, E1 F10), no notification producer exists (`notify_user` has zero callers), there is no on-call and
no dashboard. A rogue Admin's most effective hiding place is _the absence of a record_, and the app provides
it by default.

### 8.2 What a low-privilege account does that it should not

**Moderator** (assignable by any Admin — so "get a friend promoted to Moderator" is the cheapest insider path):

- delete **any** user's media from R2 (irreversible, unlogged) — `media_v1/controller.rs:543-548`;
- read **any** user's media + owner email — `:539-541`, `:763`;
- bypass the paywall on every paid/subscriber-only post — `post_v1/controller.rs:35-40`;
- read all drafts and all comments on unpublished/hidden posts — `:407-418`, `post_comment_v1/controller.rs:44-46`;
- attach another user's media to their own post — `post_v1/controller.rs:58`.

**Author**:

- `post_v1::delete` and `revisions_restore` on their own posts (by design), but note `media_v1::delete`
  performs **no usage check**, so an Author can delete their own media that a _live published post_ still
  references → the post renders with a dead image (the FK nulls the reference; the R2 object is gone);
- `media_v1::create` is the only upload path and image moderation is **absent** by default (E1 F5) — an
  Author is a trusted role but the classifier the runbook might rely on does not exist;
- `post_v1::query` is correctly scoped to their own posts (`controller.rs:407-410`) — no cross-author read.

**User** (role 0): correctly denied on all of the above; the only lever is `comment:create` abuse-limited
20/60 s per account.

---

## 9. Minimum fix set, ordered

Reasoning for the order: **evidence first** (it is the precondition for reviewing anything else and needs no
migration), then **the actions that destroy evidence or mint credentials**, then the cheap structural
limits, then read logging. Everything deferred has a stated reason and a trigger that would un-defer it.

### P0 — this week (all in-repo, ~1–2 dev-days total)

| #   | Fix                                                                             | Why first                                                                                                                                                                                                                                                        |
| --- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **`services/audit.rs` + wire the destructive set** (§4.3, rows 1–16)            | The table exists, no migration needed, and every other finding is unverifiable without it. Same-transaction writes for destructive actions. ~150 LOC.                                                                                                            |
| 2   | **Seed prod guard + no seeded Admins** (§5.4.1–5.4.2)                           | One command away from a guessable admin account. `is_production()` refusal in `seed_now`, the TUI, and `seed_all`/`seed_custom`; delete the `password = email` + Admin-role pattern.                                                                             |
| 3   | **Comment-delete guard** (§2.2)                                                 | Refuse `admin_delete` when `flags_count > 0` unless `confirm_evidence=true`; make `admin_flags_clear` write a `cleared_at` marker (or capture first); delete the dead `comment_flag::clear_flags`. ~20 LOC. This is E1's own fix and it is the live UGC surface. |
| 4   | **User-delete guard + pre-state snapshot** (§2.1)                               | Require the target's email typed in the body; write the pre-delete snapshot (email, role, counts of what will cascade) into `audit_logs` **before** the delete; refuse when the target has flags against them or an open report. ~40 LOC.                        |
| 5   | **Nest limiters on the ten unmetered nests** (§6)                               | One line each; closes the only unauthenticated DoS (feed) and the operator-nest hammering.                                                                                                                                                                       |
| 6   | **Typed confirmation on TUI undo + stop range-deleting `users`** (§5.4.3–5.4.4) | Before anyone adds `seed_runs` (C3's fix) and arms it.                                                                                                                                                                                                           |

### P1 — next (P0 is the floor, not the ceiling)

| #   | Fix                                                                   | Why now / why not P0                                                                                                                                                                                 |
| --- | --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 7   | **Read logging for Class A/B reads** (§7)                             | Needs the writer from #1 to exist; cheap afterwards. Required by A5/B9.                                                                                                                              |
| 8   | **Category-delete guard** (§2.3)                                      | Show the post count and require confirmation; or convert to archive. Cheap once the audit writer exists; the cascade is already destructive but requires an admin to aim it at a populated category. |
| 9   | **Media-delete usage check** (§2.4)                                   | Refuse (or warn) when `media_usage` is non-empty; log the object key + hash.                                                                                                                         |
| 10  | **Mail suppression guard** (§1.9)                                     | Require a reason + write an audit row; make the suppressed-transactional path record `notice_delivery_state` instead of a bare `Ok` (E1 F9).                                                         |
| 11  | **Ban-creation path** (`user_bans` write + notice in one transaction) | E1 F8. Not P0 for _this_ audit because the destructive tool today is delete, and #4 makes delete reviewable; but without a ban the only enforcement is deletion, which is the root of §2.1.          |
| 12  | **Route-block self-lockout guard**                                    | Refuse blocking the unblock/list routes; log the block with the reason.                                                                                                                              |

### P2 — defer, with the trigger that un-defers

| Fix                                                                                  | Defer reason                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Moderator gating (queue at ROLE_MODERATOR)**                                       | There is no second human; gating a queue nobody else reads adds nothing. Un-defers the day a second operator exists — and _before_ promoting them, narrow the inline `is_moderator()` powers (#13) or the promotion silently grants media deletion + email reads. |
| **Narrowing `is_moderator()` powers** (media delete, paywall bypass, email exposure) | Real design question, not a bug: decide whether "moderator" means "content reviewer" or "media admin". Document whichever, and gate the dangerous two (`media delete`, `media usage emails`) at `is_admin()`. Cheap, but it is a product decision.                |
| **Separate audit table for reads / hash chain**                                      | `audit_logs` is enough for v1 (A8 F5 agrees); a chain is for contested disputes and there are none (C3 base rate 0).                                                                                                                                              |
| **`app_constants` wiring or unmounting**                                             | The module has no runtime effect (no reader) → it is dead config, not a live risk. Fix when something actually needs runtime config; log the writes now (#1).                                                                                                     |
| **Billing admin routes**                                                             | Tables absent (D5 2a) → every route 500s. Fix with the billing decision, not before.                                                                                                                                                                              |
| **Notification per-recipient cap**                                                   | Admin-only producer, zero automated producers (D5 item 3) → not user-reachable.                                                                                                                                                                                   |

---

## 10. Findings

All **NEW** unless marked otherwise. Severity is the admin-surface severity, not the app-wide severity.

**F1 — The seed system creates verified Admin accounts whose password equals their email, and neither the CLI nor the TUI refuses to run against production. NEW. High.**
Evidence: `services/seed/base.rs:171-188` (`password = user.email.clone()`, `role = Admin` for
`rng.random_bool(0.1)`, `is_verified = Some(true)`); `services/seed/custom.rs:135-155` (same, 1-in-3 Admin);
`scripts/seed_now.rs:5-24` (reads `DATABASE_URL`, no `is_production()` check, `.env.dev` fallback);
`tui/app.rs:887-897` (`run_tui` → `try_connect(true)`); `Cargo.toml:192-200` (bins need `seed-system`);
`images.yml:187-192` (image builds without the feature — the only reason this is latent).
D5 item 5 covered "no production guard" and "undo deletes ID ranges"; the _credential_ consequence
(password == email, Admin role, verified, fixed `FreeEmail()` word list) is not in any prior report.
Fix: §5.4.

**F2 — `audit_logs` has a table, a model, and zero writers; 20 classes of admin mutation and every sensitive read are unrecorded. NEW (C3 F7 named five actions; this enumerates 20).**
Evidence: `db/sea_models/audit_log/` contains only `model.rs` + `mod.rs`; `rg 'audit_log|AuditLog|write_audit' src/` → model + registration only; `m000001_init.rs:252-264` (schema, `user_id` FK `SET NULL`).
C3 F7 listed route block, flags_clear, hide/unhide/delete, the ban path; **not** user delete, user
create/update, password reset, post delete, media delete, ban lift, ACL writes, newsletter send, suppression
writes, verification issue/delete, category/tag mutations, notification create, seed runs, or any read.
Fix: §4.3.

**F3 — Deleting a comment cascades its `comment_flags` — the report evidence — and FKs are enforced per pooled connection, so this is live. NEW (verifies E1 F1 with the enforcement proof).**
Evidence: `post_comment/actions.rs:384-387` (`delete_by_id`); `m000001_init.rs:298-308`
(`comment_flags.comment_id` + `.user_id` `ON DELETE CASCADE`); sqlx-sqlite 0.8.6 `src/options/mod.rs:185` +
`src/options/connect.rs:39` (sqlx re-issues `PRAGMA foreign_keys = ON` on every new pooled connection, so the
8-second `max_lifetime` in `sea_connect.rs:40` does **not** disable cascades); `comment_flag/actions.rs:179-186`
(dead `clear_flags` = `delete_many`, zero callers); `post_comment/actions.rs:389-400` (`flags_clear` zeroes the
count while the rows survive).
Fix: §9 #3.

**F4 — `user_v1::admin_delete` is a hard delete whose CASCADE reaches the user's ban, sessions, posts, comments and every flag they filed; no snapshot, no soft delete, no notice. NEW (E1 F2 states the mechanism; this enumerates the full chain and the orphans).**
Evidence: `user_v1/controller.rs:269-318`; `user/actions.rs:486-497`; the CASCADE graph in §2.1
(`m000001_init.rs:55-56,67-68,119-120,132-133,142-143,162-163,172-173,182-183,190-193,271-272,280-281,293-296,304-307`;
`m000005:21-22,37-38,53-54`; `m000007:19-20,34-35`; `m000010:25-26`); `media.uploader_id` `SET NULL` leaves
orphaned media rows and never reclaims the R2 objects.
Fix: §9 #4 + #11.

**F5 — Deleting a category cascades every post in it, and each post's delete cascades its comments and their flags. NEW (no prior report covers the category cascade).**
Evidence: `category_v1/mod.rs:13-20` (delete at ROLE_ADMIN); `m000001_init.rs:121-122`
(`posts.category_id` `ON DELETE CASCADE`); `post_v1/controller.rs:289-311` (post delete); the post cascade in §2.3.
Fix: §9 #8.

**F6 — An Admin can mint unlimited Admin peers and promote any lower user to Moderator; no approval, no notification, no record. NEW (D5 item 10 says "role guards are correct" — they are correct against _vertical_ escalation and silent about peer-minting).**
Evidence: `user_v1/controller.rs:249-259` (create: `requested > caller_level` is the only cap),
`:333-346` (update: same), `enums/user.rs:26-34` (levels). The Moderator consequence is F7.
Fix: audit the role transitions (§4.3), and decide whether Admin-minting needs SuperAdmin approval or a
notification to the SuperAdmin.

**F7 — `ROLE_MODERATOR` is not dead: no route gate uses it, but inline `is_moderator()` grants delete-any-media, read-any-email, and paywall bypass. NEW.**
Evidence: `user/model.rs:49-51` (`role >= 2`); `media_v1/controller.rs:539-548` (view/delete any media),
`:763` (owner email for staff); `post_v1/controller.rs:35-40` (paywall bypass), `:121` (`is_staff`),
`:407-418` (all posts incl. drafts); `post_comment_v1/controller.rs:44-46` (hidden posts' comments);
`post_v1/controller.rs:58` (others' media attachable). Assignable via `admin_update` (F6).
Fix: before anyone is promoted to Moderator, decide and gate: media delete + email exposure at `is_admin()`,
or accept and document.

**F8 — `admin_change_password` is a silent account takeover of any strictly-lower user: no fresh-auth requirement on the caller, no notice to the target, and the target's sessions are revoked so they may not notice. NEW.**
Evidence: `user_v1/controller.rs:385-417` (rank check only); `user/actions.rs:207-240`
(`session_auth_secret` rotation); contrast `user_v1/controller.rs:222-236` (self-service email change _does_
require `fresh_auth::consume`).
Fix: require fresh auth for admin password resets, notify the target (mail + in-app), and audit the event.

**F9 — Ten nests have no rate limiter, including four admin/operator nests and the only unauthenticated DoS (the RSS/Atom feed). NEW (D5 named four nests; the list is ten).**
Evidence: `router.rs:125-158` (`/category`, `/tag`, `/mail`, `/feed`, `/analytics`, `/admin/route`,
`/admin/acl`); `admin_bans_v1` merged with no layer (`router.rs:150-153`); `router.rs:82-83`
(`/email_verification`, `/forgot_password`); `main.rs:827-858` (no global limiter);
`feed_v1/mod.rs:135-151` (per-request `posts` query + `load_post_access_map` on a pool of 1).
Fix: §9 #5.

**F10 — The route blocker is an unlogged global availability switch that can block its own unblock route. NEW (D5 lists the route-blocker actions as unlogged; the self-lockout and pre-auth scope are new).**
Evidence: `admin_route_v1/controller.rs:22-133` (block/unblock/update/delete, `info!` only);
`admin_route_v1/validator.rs:7-17` (any 1–255-char pattern, no denylist);
`middlewares/route_blocker.rs:60-118` (exact MatchedPath match, applied to the whole private router
**before auth**, `/healthz` the only exemption); `route_blocker_service.rs:165-180` (delete removes the record).
Fix: §9 #12.

**F11 — No sensitive read is logged: user PII, reporter identity, media-owner email, subscriber and suppression lists, ban export. NEW (A5/B9 require operator read access to device/account data be logged; no prior report names the read set).**
Evidence: `user_v1/controller.rs:421-461` (`admin_list`/`admin_view`); `post_comment_v1/controller.rs:389-468`
(flags list/summary/details — reporter identity + free text); `media_v1/controller.rs:550-631,634-801`
(any media + owner email); `admin_bans_v1/controller.rs:163-181` (export); `newsletter_v1/controller.rs:243-269`;
`mail_v1/controller.rs:27-41`. Also folded in: `admin_acl_v1` is an unaudited secret store with **no runtime
reader** and **client-declared** masking (`admin_acl_v1/controller.rs:114-160` sets `is_sensitive` from the
request body; `acl_service.rs:37-74` imports env vars by name heuristic; `rg 'AppConstant::'` → the ACL
service only) — Low severity because it has no effect, but the write is unrecorded.
Fix: §7.

**F12 — The mail suppression list is a per-address authentication kill switch, and the operator mail surfaces (newsletter send, `admin/issue_code`) are unmetered senders. NEW (E1 F9 covers the silent suppressed-transactional drop; the admin-facing kill switch and the unmetered senders are new).**
Evidence: `mail_v1/controller.rs:43-77` (add/remove any address, ROLE_ADMIN, no limiter, no audit);
`mail/router.rs:251-259` (suppressed transactional → `debug!` + `Ok`); `mail/provider.rs:17-22`
(transactional = verification + password_reset only, so suppression silently disables account recovery);
`newsletter_v1/controller.rs:152-239` (send to every confirmed subscriber, no cap/cancel/confirm, background
task); `email_verification_v1/controller.rs:246-286` (`admin/issue_code` emails any target; no rank check —
a SuperAdmin's address can be bombed); `router.rs:82-83` (no limiter on that nest).
Fix: §9 #10 + audit both senders.

**F13 (verified, not a finding) — `foreign_keys=ON` is applied by sqlx on every pooled connection, so the 8-second `max_lifetime` in `sea_connect.rs:40` does not silently disable the CASCADE graph.** Recorded because it is the one assumption the entire destruction audit rests on:
sqlx-sqlite 0.8.6 `src/options/mod.rs:185` inserts `foreign_keys = ON` into the default pragma map and
`src/options/connect.rs:39` executes the pragma string inside `SqliteConnectOptions::connect()`, i.e. per
connection; `src/lib.rs:135` does the same on establish. The app's own `configure_sqlite`
(`sea_connect.rs:46-56`) is therefore redundant for FKs (and applies `journal_mode` once, which is persistent
per file anyway).

---

## 11. Verification commands (all read-only)

```bash
# admin routes + their auth + their rate limits
rg -n "ROLE_ADMIN|ROLE_SUPER_ADMIN|ROLE_MODERATOR" rust/backend/api/src/modules
rg -n "nest\(|merge\(|rate_limit_layer" rust/backend/api/src/router.rs
sed -n '827,858p' rust/backend/api/src/main.rs                    # private stack: no global auth/limiter

# the audit gap
rg -n 'audit_log|AuditLog|write_audit' rust/backend/api/src        # model + mod only
sed -n '252,264p' rust/backend/api/migration/src/m000001_init.rs   # audit_logs schema

# the cascade chain (E1 F1 + the user/category deletes)
sed -n '283,308p' rust/backend/api/migration/src/m000001_init.rs   # post_comments + comment_flags CASCADE
sed -n '103,125p' rust/backend/api/migration/src/m000001_init.rs   # posts: author_id + category_id CASCADE
sed -n '48,73p'   rust/backend/api/migration/src/m000001_init.rs   # user_sessions + user_bans CASCADE
sed -n '384,400p' rust/backend/api/src/db/sea_models/post_comment/actions.rs
sed -n '486,497p' rust/backend/api/src/db/sea_models/user/actions.rs
rg -n "foreign_keys" ~/.cargo/registry/src/index.crates.io-*/sqlx-sqlite-0.8.6/src/options/mod.rs

# seed / TUI
sed -n '171,203p' rust/backend/api/src/services/seed/base.rs       # password = email, 10% Admin
sed -n '135,162p' rust/backend/api/src/services/seed/custom.rs
sed -n '29,105p'  rust/backend/api/src/services/seed/undo.rs       # del_range! over 19 tables incl. users
sed -n '613,634p' rust/backend/api/src/tui/app.rs                  # one-keypress undo
sed -n '187,192p' .github/workflows/images.yml                     # --features vendored-openssl (no seed-system)
sed -n '9,21p'    rust/backend/api/Cargo.toml                      # default = ["openapi"]

# the rank guards
sed -n '240,265p;269,318p;322,383p;387,417p' rust/backend/api/src/modules/user_v1/controller.rs
sed -n '539,548p' rust/backend/api/src/modules/media_v1/controller.rs  # is_moderator can delete any media
sed -n '35,44p'   rust/backend/api/src/modules/post_v1/controller.rs   # is_moderator bypasses the paywall
```
