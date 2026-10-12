# G2 — Bookmark-sync amplification and UGC lifecycle bounds (round 8)

Auditor: sync/UGC lifecycle auditor (round 8). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree, read-only). Method: read `F6-remaining-gaps.md` (hole 2), `C1-optionAB-audit.md`,
`B4-rules.md` (R10/R11), `F3-admin-surface.md` (the destruction chain), `E1-incident-response.md`
(§0/§3); then read the actual code listed in §0 below. Every `file:line` in this report was read this
session. No repo writes, no DB writes, no network.

**Verdict up front.** `F6` §2.2 is **confirmed, and the numbers are worse than stated in one dimension
and different in another**:

1. The folder vector is real: **200 mutations/round × 100 rounds/min = 1.2M folder rows/hour from one
   verified account on one IPv4**, ~340 MB/hour durable, and the snapshot response the same account
   receives grows to **~216 MB after one hour and ~660 GB of response bytes inside the first hour**.
2. The bookmark bound is **32,604 rows/account** (114 × 286), not the 6,236 real verses — the validator
   accepts `surah=1, ayah=286` (`validator.rs:52-55`). The row count is still bounded, but the cap must
   be stated correctly.
3. The UGC lifecycle side (post/comment/flags/likes/views) has **no per-account caps anywhere**, **no
   moderation on either publish path**, **zero indices on ten of these tables**, and one **live
   moderation-integrity bug** (`admin_flags_clear` is silently reverted by the next flag) that no prior
   report states. The comment surface is **flat — there is no nesting and no reply table at all**, so
   the "nesting depth cap" item is answered "mechanism absent", not "uncapped".
4. `B4` R11 (cross-account duplicate content) **cannot be implemented from the current schema**: there
   is no body-hash column on `posts`/`post_comments`, no "human-confirmed violating" record, and
   `audit_logs` still has zero writers. R11 as written needs new storage.

Fix set in §8, self-attack in §9, defer list in §10, 12 findings in §11 (all **NEW**).

---

## 0. Ground truth — what I read

| Area                | Files                                                                                                                                                                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bookmark module     | `rust/backend/api/src/modules/bookmark_v1/{mod,controller,validator}.rs`                                                                                                                                                                                      |
| Bookmark models     | `rust/backend/api/src/db/sea_models/bookmark/{actions,model,slice}.rs`, `bookmark_folder/{actions,model,slice}.rs`                                                                                                                                            |
| Bookmark migrations | `rust/backend/api/migration/src/m000007_bookmarks.rs`, `m000008_bookmark_verse_unique.rs`                                                                                                                                                                     |
| Web sync            | `web/src/lib/sync/{engine.svelte.ts,outbox.ts,types.ts,index.ts}`, `web/src/lib/bookmarks/{domain.ts,schema.ts,store.svelte.ts}`                                                                                                                              |
| Post module         | `rust/backend/api/src/modules/post_v1/{mod,controller,validator}.rs`                                                                                                                                                                                          |
| Post models         | `db/sea_models/{post,post_revision,scheduled_post,post_series,post_series_post,post_access,post_purchase}/{actions,model,slice,mod}.rs`                                                                                                                       |
| Comment module      | `rust/backend/api/src/modules/post_comment_v1/{mod,controller,validator}.rs`                                                                                                                                                                                  |
| Comment models      | `db/sea_models/{post_comment,comment_flag,post_like,post_view}/{actions,model,slice}.rs`                                                                                                                                                                      |
| Scheduler / paywall | `src/services/{scheduler,paywall,abuse_limiter}.rs`                                                                                                                                                                                                           |
| Limits / bodies     | `src/router.rs`, `src/config/mod.rs` (`body_limits`), `src/main.rs` (private router stack), `src/modules/*/mod.rs`, `src/middlewares/{rate_limit,auth_guard,client_ip}.rs`, `crates/rux-request-gate/src/{layer,abuse,ip}.rs`, `tests/request_body_limits.rs` |
| Schema              | `migration/src/m000001_init.rs` + `grep 'CREATE INDEX' migration/src/m0000*.rs`                                                                                                                                                                               |

---

## 1. Bookmark sync — F6 hole 2, verified

### 1.1 The mechanism, line by line

- **Request cap.** `validator.rs:63-65` — `mutations: Option<Vec<SyncMutation>>` with
  `#[validate(length(max = 200), nested)]`. 201 → 422, whole round rejected
  (pinned by `tests/bookmarks_v1.rs::validation_errors_reject_the_round`).
- **Field caps.** `validator.rs:46-57` — `id` must be a dashed-hex UUIDv4 (36 chars), `name` ≤ 100,
  `folder_id` UUID, `surah` 1..=114, `ayah` 1..=286, `updated_at` any RFC3339 (client-supplied).
- **No per-user cap on folders.** `bookmark_folder/actions.rs:31-61` — `upsert_lww` is a bare
  `INSERT … ON CONFLICT (id) DO UPDATE` guarded only by the LWW timestamp and the ownership equality.
  There is no `COUNT` check anywhere on the folder write path (grep `COUNT` over
  `db/sea_models/bookmark_folder/` → 0 hits).
- **No per-user cap on bookmarks either — only the accidental verse-collapse.** `bookmark/actions.rs:48-159`
  deletes rival rows on the target verse (step 3b) before inserting, and
  `m000008_bookmark_verse_unique.rs:52-54` enforces `UNIQUE (user_id, surah, ayah)`. So the bookmark
  row count is bounded by the **reachable `(surah, ayah)` space = 114 × 286 = 32,604** (not 6,236 —
  the validator does not know per-surah ayah counts).
- **Unbounded snapshot.** `bookmark_v1/controller.rs:103-120` — the comment says _"Snapshot is unbounded
  by design"_; both `list_for_user` calls are unbounded (`bookmark_folder/actions.rs:104-111`,
  `bookmark/actions.rs:187-194`), each `ORDER BY updated_at ASC` with only a `user_id` index
  (`m000007:23-24,40-44`) → a full per-user scan plus a temp b-tree sort on every round.
- **The only limiter is per-IP.** `router.rs:174-177` — `rate_limit_layer(&state, 100, 60)`. The bucket
  key is `ratelimit:{canonical unit}:{matched route}` (`crates/rux-request-gate/src/layer.rs:239-241,278`),
  and the canonical unit is **IPv4 `/32` (single address), IPv6 `/64`** (`layer.rs:378-381`). So:
  **100 rounds/min per single IPv4**, not per /24 — one address suffices, rotating addresses multiplies.
- **Auth gate.** `auth_guard.rs:57-70` — `verified` = authenticated + verified + not banned. There is no
  role requirement and no per-account throttle; `expected_user_id` (`controller.rs:131-136`) is an
  optional _client_ account-change detector, not a cap.
- **The transaction window.** `controller.rs:149-157` — one `begin()` wraps the mutation apply **and**
  both snapshot SELECTs; `commit()` happens before the `json!` at `:159-166`. So the single pooled
  connection is held across a full scan + sort of every row the user owns, and the response tree is
  built after release.

### 1.2 The amplification, computed

Let _n_ = average folder name length.

| Quantity                                                                                                | Formula                                                                                | Value                                        |
| ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | -------------------------------------------- |
| Rows pushed per round                                                                                   | ≤ 200                                                                                  | 200                                          |
| Rounds per minute, one IPv4                                                                             | 100 (`router.rs:176`)                                                                  | 100                                          |
| **Rows per hour**                                                                                       | 200 × 100 × 60                                                                         | **1,200,000**                                |
| Folder row on disk (row + TEXT-PK autoindex + `idx_bookmark_folders_user`)                              | ≈ 130 + n + ~90                                                                        | **~220–320 B** (n = 15…100)                  |
| **Durable growth per hour**                                                                             | 1.2M × ~280 B                                                                          | **≈ 340 MB/h** (range 260–460 MB/h)          |
| Folder in the snapshot JSON (camelCase: id 36 + name n + two RFC3339-micros stamps 27+27 + punctuation) | 142 + n                                                                                | **~157–242 B** (~180 B typical, 245 B worst) |
| **Snapshot size after h hours of pushing**                                                              | 1.2M·h × 180 B                                                                         | **≈ 216 MB·h** (worst 294 MB·h)              |
| **Response bytes served in the first hour**                                                             | Σ over the 100 rounds of each minute: 100 × (20k·m) × 180 B                            | **≈ 660 GB** (worst ~900 GB)                 |
| Peak memory for one request at h = 1                                                                    | 216 MB JSON + a `serde_json::Value` tree ~2–3× that + the `Vec<Model>` from the SELECT | **≈ 0.5–0.8 GB**                             |

Precision on the headline figures:

- After one hour the single response is **~216 MB** with a 15-char name and **~294 MB** with 100-char
  names; the 100 MB mark is crossed at **~28 minutes** of pushing.
- Durable growth is **~340 MB/h** at n = 15 and **~460 MB/h** at n = 100.

The resource failure is not just disk: at h = 1 the round that returns 1.2M rows builds a
~0.5–0.8 GB object graph on a documented ≥2 GB VPS, and the round that _reads_ them holds the only
pooled connection across the scan + sort. The first hour of this is an availability event, not a
storage event. Bookmarks add a bounded ~5.9 MB (32,604 × ~180 B) on top.

**What F6 did not note:** the same endpoint is the legitimate user's mobile-data bill. A heavy
legitimate user (6,236 bookmarks + 100 folders) receives **~1.2 MB per round**, and the engine flushes
every 30 s while online plus on every mutation debounce (`engine.svelte.ts:22-28,88-90`) → **~3.4 GB/day**
of mobile traffic for a user who changes nothing. That is the strongest argument for delta sync beyond
the abuse case.

---

## 2. Conflict resolution, versioning, and the `expectedUserId` guard

**Mechanism: per-row last-write-wins on a fully client-supplied timestamp. No server versioning, no
vector clocks, no tombstones, no merge.**

- `bookmark_folder/actions.rs:16-19,42-49` and `bookmark/actions.rs:11-46,133-145` — every write is
  guarded by `julianday(incoming) > julianday(stored)`; equal timestamps are a no-op (strictly-newer).
- `bookmark/actions.rs:59-97` — two extra guards before the insert: same-id staleness (step 1) and a
  strictly-newer rival on the target verse (step 2). A skip is non-destructive by construction.
- Delete is LWW too (`delete_lww`, `folder/actions.rs:71-102`, `bookmark/actions.rs:167-185`): a delete
  older than the row is refused, and there is no tombstone — the row simply survives.

**What a user loses, precisely:**

1. **An offline edit silently dropped.** Device A edits folder `X` at 10:00 offline; device B renames it
   at 10:05; A comes online at 10:10. A's timestamp is older → `skipped`. The client replaces its whole
   state with the snapshot (`store.svelte.ts:337-344`), so A's edit vanishes and the only trace is the
   `skipped` counter, which the engine discards (`engine.svelte.ts:322-347` keeps `lastError`, not the
   skip count; `domain.ts:69-73` returns it and nothing consumes it).
2. **Clock-skew lockout.** A device whose clock is fast writes `updated_at` in the future. Every later
   edit from a correct-clock device compares _older_ and is refused until real time passes the stored
   value. +1 h skew → the account's other devices cannot change that row for an hour; **year 9999 →
   effectively forever**, and the row cannot be deleted either (`delete_lww` also requires
   `stored <= incoming`). Self-inflicted (the guards are `user_id`-scoped), but it is a permanent
   data-integrity lock on a legitimate account, and a shared/family device with a broken clock reaches it.
3. **Concurrent rename → one winner, no merge.** Two devices mint different folder ids for the "same"
   folder → **two folders with the same name** live side by side (no uniqueness on `name`). Only the
   bookmark verse is deduped, not the folder identity.
4. **`created_at` is rewritten.** A verse move is delete-then-insert (`bookmark/actions.rs:101-107`),
   so "saved on" resets to the move timestamp. Cosmetic.
5. **Replay is safe.** Identical timestamps are a no-op (`replayed_round_changes_nothing`), so
   duplicate pushes from multiple tabs are idempotent. Note: the outbox has **no cross-tab lease**
   (`outbox.ts:281-295` — `take` reads without claiming), so N tabs can push the same batch N times;
   LWW makes it correct, but it multiplies the write traffic that §1.2 measures.

**Cross-user corruption: no. The guard is not `expectedUserId` — it is the SQL ownership predicates.**

- `expectedUserId` is enforced in exactly one place: `bookmark_v1/controller.rs:131-136`, and `/sync` is
  the only route in the module (`mod.rs:8-11`), so it _is_ on every bookmark mutation. But it is
  **optional** (`payload.expected_user_id: Option<i32>`) and its purpose is client-side account-change
  detection: a mismatch returns 403 with the message the web client string-matches to suspend the domain
  and re-probe (`domain.ts:60-64`). Authorization is the session; this field grants nothing.
- The real cross-account guards are: `bookmark_folder/actions.rs:44-49` (`WHERE … user_id = excluded.user_id`
  in the DO UPDATE), `bookmark/actions.rs:61-97` (both staleness checks filtered by `user_id`) and
  `:136-145` (same DO-UPDATE guard), `resolve_folder_id` (`controller.rs:38-49`, ownership-checked),
  and `list_for_user` (`user_id` filter). Pinned by `tests/bookmarks_v1.rs::cross_user_ids_are_isolated`.
- Residual: a _nonexistent_ id is inserted fresh for whoever pushes it first — that is first-come
  ownership of a UUIDv4, not a hijack, and it cannot read or mutate another account's row.

---

## 3. Post lifecycle abuse (authenticated Author)

**Role context:** `/post/v1` writes are `ROLE_AUTHOR` + verified + not-banned (`post_v1/mod.rs:52-54`);
the role is **admin-assigned** (`E1` §0; `media_v1/mod.rs:19-21` is the same pattern), so the threat is a
rogue/compromised Author or an admin mistake, not a self-serve signup. That bounds the population but not
the blast radius.

| Object            | Cap in code                                      | Enforced at                                                                                | Abuse headroom                                |
| ----------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------ | --------------------------------------------- |
| Posts             | **none per account**                             | —                                                                                          | unlimited rows                                |
| Post body         | **256 KiB**                                      | `DefaultBodyLimit::max(body_limits::POST)` (`post_v1/mod.rs:45`, `config/mod.rs:11`)       | per request                                   |
| Revisions         | **10 per post**                                  | `post_revision/actions.rs:13,35` (`MAX_REVISIONS_PER_POST`, enforced inside the insert tx) | bounded per post; **unbounded via new posts** |
| Scheduled posts   | **1 pending per post**                           | `scheduled_post/actions.rs:37-73` (upsert reuses the pending row)                          | dead table — see below                        |
| Series            | **admin-only**, no count cap, **no unique slug** | `controller.rs:618-636,638-668,670-696` (`is_admin()`); `post_series/actions.rs:15-35`     | admin only; duplicate slugs possible          |
| Series membership | idempotent add, **no cap**                       | `post_series_post/mod.rs` `add` (returns existing)                                         | admin only                                    |
| Publish           | **no moderation on any path**                    | see below                                                                                  | —                                             |

**Revisions are not free storage — posts are.** The 10-revision cap is real and enforced inside the
insert transaction, so `revisions` cannot be used as an unbounded store. But new posts are uncapped, and
each post carries 10 revisions: per post the ceiling is ~256 KiB (content) + 10 × 256 KiB (revisions) ≈
**2.8 MB**. The rate is the constraint, not the per-object cap.

**Write amplification (all three write routes have independent buckets).** `post_v1/mod.rs:40-50` puts
`/create`, `/update/{post_id}` and `/autosave` in one sub-router that carries the POST body limit, and the
nest layer keys per **matched route** (`router.rs:115-118` + `layer.rs:278`). So an Author gets **200/min on
`/create`, 200/min on `/update/{post_id}`, 200/min on `/autosave` = 600 write requests/min**, each ≤ 256 KiB:

```
600 req/min × 256 KiB = 150 MiB/min = ~9 GiB/hour of post + revision content
```

with `/autosave` writing a revision **and** rewriting the post content (`controller.rs:460-490`), i.e.
double storage per call. SQLite has no VACUUM job in-repo, so the file only grows.

**Scheduled publish: two sources of truth, and the live one has no gate.**

- `POST /post/v1/schedule` writes `scheduled_posts` (`controller.rs:599-615` → `scheduled_post/actions.rs:37-73`).
- The scheduler **never reads that table**. `services/scheduler.rs:31-36` selects `posts` where
  `status = Draft AND published_at IS NOT NULL AND published_at <= now` and flips them to Published
  (`:70-80`), with a fire-time Author re-check (`:47-68`). `scheduled_post::Entity::due_pending`
  (`actions.rs:85-102`) has **no callers** (grep: only its definition and the seed system).
- So the real schedule is `posts.published_at`, which an Author sets directly via create
  (`validator.rs:266,286-290` — `is_published` → Published instantly) or update
  (`validator.rs:308,316-322` — `status` and `published_at` both client-settable).
- Consequence: a post can be published **with no request at publish time** (create a Draft with a past
  `published_at`; the ≤60 s tick publishes it), and **no publish path runs any moderation check** — not
  the controller, not the scheduler. If a review gate is added to `/post/v1/update` only, the scheduler
  path bypasses it. There is nothing to "bypass" today because review does not exist; the finding is that
  the plan's review assumption has no seam, and the two publish paths will diverge the moment one is added.

**Paywall read path.** `paywall::user_has_access` (`services/paywall.rs:163-193`) → policy from
`post_access` (`:68-80`, `.one()` = `LIMIT 1` without ORDER BY, so duplicate rows resolve
nondeterministically) + purchase/subscription checks; the controller blanks `content` when not granted
(`post_v1/controller.rs:107-109,147-149`). Moderators and the author bypass (`:35-40`). No cap on
purchase rows (`post_purchases`), no `UNIQUE(post_id)` on `post_access` (writes are delete-then-insert
at `billing_v1/controller.rs:1187-1200`, admin-only, and the billing tables are missing per `E3`/`D5`).

---

## 4. Comment lifecycle abuse

**Nesting: absent.** `m000001_init.rs:283-297` — `post_comments` has no `parent_id`, no thread column, no
reply table anywhere in `migration/src/`. `find_all_by_post` returns a flat list capped at 500 rows
(`post_comment/actions.rs:180`) and `admin/list` paginates flat rows. So there is **no nesting depth cap
to audit**; replies are not modelled. (The `CommentWithUser` DTO carries no children either.)

| Action                           | Route / guard                                                       | Per-account limiter                                                                                           | Notes                                                                                        |
| -------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| create                           | `post_comment_v1/mod.rs:10,14` — `verified`                         | **yes**: `comment:create:user:{id}`, 20/60 s → 600 s block; 100/3600 s → 3600 s (`controller.rs:21-28,62-67`) | limiter runs **before** visibility (`:62` vs `:72`); counts successes (the `C1` F1/F2 class) |
| update                           | `mod.rs:11` — `verified` + owner (`post_comment/actions.rs:54-57`)  | **none**                                                                                                      | nest 100/min/IP only                                                                         |
| delete                           | `mod.rs:12` — `verified` + owner (`actions.rs:85-89`)               | **none**                                                                                                      | cascades own flags                                                                           |
| flag                             | `mod.rs:13` — `verified`                                            | **none**                                                                                                      | nest 100/min/IP only                                                                         |
| list                             | `mod.rs:15` — public, post-visibility gated (`controller.rs:35-51`) | —                                                                                                             | `LIMIT 500`                                                                                  |
| admin hide/unhide/delete/flags/* | `mod.rs:17-37` — `ROLE_ADMIN`                                       | nest only                                                                                                     | see `F3` §2.2                                                                                |

**Edit after flag — evidence destroyed.** `post_comment/actions.rs:48-82` loads the row by
`(id, user_id)` and overwrites `content` unconditionally: **no check on `hidden`, none on `flags_count`**,
and there is no revision table for comments. An author who is flagged can rewrite the body to something
benign; the moderator then sees the edited text, and `comment_flags.reason` (free text, ≤500 chars,
`validator.rs:86-90`) is the only surviving trace of what was reported. Same for a hidden comment: the
row stays editable.

**Delete/repost loop.** Delete (unlimited) + create (20/60 s per account) → **~1,200 posts/hour** per
account. Each delete cascades the comment's flags (`m000001_init.rs:304-305`), so the loop also **resets
`flags_count` to 0** — the moderation queue's only ordering signal (`min_flags` / `FlagFilter`,
`post_comment/actions.rs:285-297`) is wiped by deleting and reposting.

**Flag manipulation — three distinct issues.**

1. **Self-flagging is allowed.** `controller.rs:341-385` never compares `comment.user_id` with
   `user.id`. The author's own flag adds one row (+1 to `flags_count`). It is a single row per
   `(comment, user)` (`comment_flag/actions.rs:13-35` upserts the reason), so one account can inflate a
   comment by exactly 1 — the vector is not count inflation, it is _who may add the first flag_.
2. **Flag flooding is unmetered.** `flag` has no per-account limiter; the nest gives 100/min/IP, so one
   account (rotating IPs) can flag **every comment on every post** at line rate → 6,000+/hour. The admin
   queue sorts by `flags_count`/`min_flags`, so a single account can make the queue useless.
3. **`admin_flags_clear` is not durable — this is the live bug.** `post_comment/actions.rs:389-400` sets
   `flags_count = 0` while leaving the flag rows; then the next flag from _any_ user hits
   `comment_flag/actions.rs:22-25` (update the existing row) and `:37` → `sync_flags_count`
   (`:162-177`) recounts **all** surviving rows and writes the total back. So an admin "clear" on a
   comment with 20 flags reads 0 until the next flag, then jumps to 21. The clear is silently reverted
   by one request. (`comment_flag::Entity::clear_flags`, `:179-186`, is the dead `delete_many` `E1` F1
   flagged — still zero callers.)
4. **Can a user clear their own flag?** No endpoint exists (no unflag route in `mod.rs`). They can only
   update the reason, or delete the comment (which cascades the flag) and repost.

**Duplicate content (B4 R11) — not implementable as written.** R11 requires "the same `HMAC(body)` posted
by ≥3 distinct accounts in 3600 s, **where that body was previously human-confirmed violating**". The
schema has no body hash on `posts` or `post_comments` (grep `content_hash|body_hash|normalize` over
`db/sea_models/post*`, `db/sea_models/post_comment*`, `modules/post_v1`, `modules/post_comment_v1` → 0
hits; the only `content_hash` in the repo is on `media`), no confirmed-violation record
(`comment_flags` has `id/comment_id/user_id/reason/created_at` only — no state, no confirmation), and
`audit_logs` has **zero writers** (grep: only the model + `mod.rs` registration). So R11 needs: a
normalized-body hash column + a confirmed-violation store + the cluster query. The human-confirmed scoping
is not just desirable on this product, it is the only thing that keeps the rule from colliding with
canonical Quran/hadith text, which is identical by design (`B4` §4 R11, `B4` §9).

**Also worth stating:** the comment create limiter counts successes and appends on the blocked path
(`crates/rux-request-gate/src/store.rs:303-315,322`), so the `C1` F1/F2 defect exists on this path too:
20 comments in a minute blocks a real user for 10 minutes, and their retries escalate toward the 1 h tier.

---

## 5. Like and view inflation

**Likes — integrity relies on application logic, not the schema.** `m000001_init.rs:185-194` creates
`post_likes` with no `UNIQUE (post_id, user_id)`. `post_like/actions.rs:20-83` does check-then-insert
inside a transaction, so with the pool of one connection (`sea_connect` max_connections = 1, per `F3` §0)
the race is closed in practice — but any pool growth silently re-opens it, and duplicates then make
`likes_count` exceed the row count. Inflation by one account is bounded: 1 like per post, and
like/unlike cycles net to zero (`:120-123` clamps at 0). `like_status_batch` is capped at 100 ids
(`post_like/slice.rs:35-41`). So the like vector costs one account per unit and is not the cheap one;
the missing unique index is the finding (integrity + future-proofing).

**Views — cheap row flooding, bounded per-post inflation.** `track_view` is public
(`post_v1/mod.rs:63-67`; `controller.rs:346-386`): no auth, nest 200/min/IP (matched route).

- Per-post dedup is **in-memory only**, key `trackview:{post_id}:{ip}`, TTL 300 s
  (`controller.rs:355-367`), and `dedup_nx` **fails open** on store error
  (`crates/rux-request-gate/src/abuse.rs:127-138`, tested at `:205-207`). So one IP inflates one post by
  at most 12/hour, but the count is lost on restart and unprotected when the store errors.
- **Row flooding is not deduped.** The key includes `post_id`, so one IP can insert **200 rows/min** by
  iterating post ids — **288k rows/day/IP** of `post_views`, each carrying raw `ip_address` + raw
  `user_agent` (`post/actions.rs:616-623`). That is unbounded PII growth and the evidence table `E1` §3
  depends on, written by an unauthenticated route.
- `post_view::has_viewed_recently` (`post_view/actions.rs:79-97`) — a real DB-backed 1-day dedup — is
  **never called** by the controller (grep: definition only). The stronger dedup exists and is dead.
- `view_count` itself is vanity (`posts.view_count`, incremented in the same tx, `post/actions.rs:635-643`).

---

## 6. Body and field caps — each value and its enforcement point

| Limit                                                                                                                                                                                     | Value                                                                 | Enforced at                                                                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default request body (all nests without an override: `/bookmark/v1`, `/post/comment/v1`, `/user/v1`, `/auth/v1`, `/notification/v1`, `/device/v1`, `/category/v1`, `/tag/v1`, `/admin/*`) | **2 MiB**                                                             | axum default (`axum-core 0.5.6 src/ext_traits/request.rs:319` `DEFAULT_LIMIT = 2_097_152`); **no** `DefaultBodyLimit` in `main.rs` or those modules (grep) |
| Post create / update / autosave body                                                                                                                                                      | **256 KiB**                                                           | `post_v1/mod.rs:45` → `config/mod.rs:11`                                                                                                                   |
| Media upload body                                                                                                                                                                         | 2 MiB                                                                 | `media_v1/mod.rs:11` → `config/mod.rs:12`                                                                                                                  |
| Billing / OAuth providers / mail body                                                                                                                                                     | 64 KiB                                                                | `config/mod.rs:10`; per-module `.layer(DefaultBodyLimit::max(…DEFAULT))` (grep list)                                                                       |
| Bookmark mutations per round                                                                                                                                                              | **200**                                                               | `bookmark_v1/validator.rs:63-65`                                                                                                                           |
| Folder name                                                                                                                                                                               | **100**                                                               | `validator.rs:48-49` (client mirror `schema.ts:14`)                                                                                                        |
| Bookmark verse                                                                                                                                                                            | surah 1–114, ayah 1–286                                               | `validator.rs:52-55`                                                                                                                                       |
| Bookmark `updated_at`                                                                                                                                                                     | **unbounded RFC3339**                                                 | `validator.rs:57` (no skew check)                                                                                                                          |
| Comment content (create + update)                                                                                                                                                         | **1,000 chars**                                                       | `post_comment_v1/validator.rs:13,30`                                                                                                                       |
| Flag reason                                                                                                                                                                               | **500 chars**                                                         | `validator.rs:88-89`                                                                                                                                       |
| Post title / slug                                                                                                                                                                         | 3–255                                                                 | `post_v1/validator.rs:262,269,304,310`                                                                                                                     |
| Post excerpt                                                                                                                                                                              | 500                                                                   | `validator.rs:271-272,312-313`                                                                                                                             |
| Post content                                                                                                                                                                              | **no per-block or block-count cap**; bounded only by the 256 KiB body | `validator.rs:37-248` validates block _shape_ only                                                                                                         |
| Series name / slug / description                                                                                                                                                          | 255 / 255 / 500                                                       | `validator.rs:394-399,404-409`                                                                                                                             |
| Like-status batch                                                                                                                                                                         | 100 ids                                                               | `post_like/slice.rs:35-41`                                                                                                                                 |
| Comment admin list / flag list page                                                                                                                                                       | **unbounded page number**                                             | `post_comment_v1/validator.rs:45,94` (the post query clamps to 500 at `post_v1/validator.rs:360`; these do not)                                            |

---

## 7. Indices — what these tables need and do not have

`m000001_init.rs` contains **no `CREATE INDEX` statement at all** (lines 9–309 are all `CREATE TABLE`).
The complete index inventory of the migration set is: `user_sessions`/auth-binding (`m000004`),
`devices`/`notifications`/`user_oauth_identities` (`m000005`), `email_verifications`/`forgot_passwords`
(`m000006`), bookmarks (`m000007`), the bookmark verse unique (`m000008`), passkeys (`m000010`),
`devices.token` (`m000011`). **Every UGC table is unindexed** apart from the implicit rowid PKs.

| Table                            | Query pattern in code                                                                                                                                                                         | Index needed                                                                                   |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `post_comments`                  | public list `WHERE post_id AND hidden=0 ORDER BY created_at LIMIT 500` (`actions.rs:177-180`); admin list filters `user_id`, `hidden`, `flags_count`, `created_at`, `updated_at` (`:263-310`) | `(post_id, hidden, created_at)`, `(user_id)`, `(flags_count)`                                  |
| `comment_flags`                  | `WHERE comment_id` count + list (`comment_flag/actions.rs:150-177`); `WHERE comment_id AND user_id` (`:14-18`)                                                                                | **`UNIQUE (comment_id, user_id)`**, `(user_id)`                                                |
| `post_likes`                     | `WHERE post_id AND user_id` (`post_like/actions.rs:10-27,89-93`); `count_by_post` (`:197-205`)                                                                                                | **`UNIQUE (post_id, user_id)`**                                                                |
| `post_views`                     | `WHERE post_id` list/count (`post_view/actions.rs:32,89-95,99-106`); `WHERE post_id AND ip_address AND created_at >=` (`:89-95`)                                                              | `(post_id, created_at)`, `(post_id, ip_address, created_at)`                                   |
| `post_revisions`                 | `WHERE post_id` count + `ORDER BY created_at` (`post_revision/actions.rs:53-56,76-92`)                                                                                                        | `(post_id, created_at)`                                                                        |
| `scheduled_posts`                | `WHERE post_id AND status` (`actions.rs:45-51,76-82`); `WHERE status AND publish_at <=` (`:90-94`)                                                                                            | `(post_id, status)`, `(status, publish_at)`                                                    |
| `post_access`                    | `WHERE post_id` (`paywall.rs:72-75`); `WHERE post_id IN` (`:90-93`)                                                                                                                           | **`UNIQUE (post_id)`**                                                                         |
| `post_purchases`                 | `WHERE user_id AND post_id` (`paywall.rs:106-112`); `WHERE user_id AND post_id IN` (`:124-129`)                                                                                               | `(user_id, post_id)`, `(post_id)`                                                              |
| `posts`                          | public list `WHERE status ORDER BY updated_at` (`post/actions.rs:573-591`); search by `author_id`/`category_id`/`published_at`/`title` (`:429-468`); sitemap `WHERE status` (`:596-606`)      | `(status, updated_at)`, `(author_id, created_at)`, `(category_id)`, `(status, published_at)`   |
| `post_series_posts`              | `WHERE series_id` count (`controller.rs:714`); `WHERE series_id AND post_id` add (`mod.rs` `add`)                                                                                             | **`UNIQUE (series_id, post_id)`**, `(post_id)`                                                 |
| `post_series`                    | `WHERE slug` (`post_series/actions.rs:97-104`); list order by `updated_at` (`:69-73`)                                                                                                         | **`UNIQUE (slug)`**                                                                            |
| `bookmark_folders` / `bookmarks` | snapshot `WHERE user_id ORDER BY updated_at` (`*_folder/actions.rs:104-111`, `bookmark/actions.rs:187-194`)                                                                                   | `(user_id, updated_at)` — needed only if the snapshot is not capped; the caps make it optional |

On a pool-of-1 SQLite connection, each missing index is a full scan serialized against every other
request. The `post_comments` public list is the worst: it is the hot reader path for every post page, and
it scans + sorts the whole table with a `LIMIT 500`.

---

## 8. The fix set, ordered

### P0 — bound the amplification (all in-repo, ~1–2 dev-days)

| #     | Change                                                  | Exact code                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----- | ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1** | **Per-user folder cap (500) and bookmark cap (10,000)** | `bookmark_folder/actions.rs::upsert_lww`: pre-check `owned_by_user`; if the id is **new** and `COUNT(*) WHERE user_id = ?` ≥ cap → return a distinct "capped" outcome; if the id exists, upsert normally (see §9.1 for why the naive `INSERT … WHERE count < cap` breaks renames). Mirror in `bookmark/actions.rs::upsert_lww` for the total row count. Surface the outcome: extend the round result with `rejected: Vec<String>` (ids) and a `capped: bool`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **2** | **Clamp `updated_at` skew**                             | `bookmark_v1/controller.rs`: before `apply_mutation`, `let ts = m.updated_at; if ts > now + 24h { ts = now }`; reject (>422) only beyond `now + 365d` so a malicious year-9999 value fails loudly while a wrong device clock is repaired silently. Applies to all four mutation kinds.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| **3** | **Snapshot size guard + honest truncation contract**    | `controller.rs::sync_round`: after the two reads, if `folders.len() > FOLDER_CAP \|\| bookmarks.len() > BOOKMARK_CAP` → `507` (unreachable once #1 lands; a belt). Never truncate silently. If truncation is ever needed, add `truncated: true` **and** make the client refuse to apply (`store.svelte.ts:337-344` replaces state wholesale → a partial snapshot wipes local rows).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| **4** | **Per-account sync limiter**                            | `controller.rs::sync`: `abuse_limiter::limiter(&state.gate_store, &format!("bookmark:sync:user:{}", user.id), SYNC_ABUSE_CONFIG)` with 60/60 s → 600 s, **no long tier** (the `C1` F2 lesson). Keep the 100/min/IP nest layer.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **5** | **Refuse comment edit after flag**                      | `post_comment/actions.rs::update`: after loading the row, `if comment.hidden \|\| comment.flags_count >= 3 { return Err(OperationNotAllowed) }` (threshold 3, not 1 — see §9.4). Error copy must say "this comment is under review and cannot be edited; delete is available".                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| **6** | **Make `admin_flags_clear` durable**                    | Migration (next free number): `ALTER TABLE post_comments ADD COLUMN flags_cleared_at TEXT` (NULL default only — the `A10` F9 rule). `admin_flags_clear` sets it to `now` with the count; `comment_flag::sync_flags_count` counts `WHERE comment_id = ? AND (flags_cleared_at IS NULL OR created_at > flags_cleared_at)`. Also bump `comment_flags.created_at` on the update path (`comment_flag/actions.rs:22-25`) or the re-flag stays invisible (§9.5).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **7** | **Missing indices + integrity uniques (one migration)** | Next free migration (SQLite, `IF NOT EXISTS`, dedupe before any UNIQUE, per `m000008`'s pattern): `idx_post_comments_post_visible(post_id, hidden, created_at)`, `idx_post_comments_user(user_id)`, `idx_post_comments_flags(flags_count)`, `idx_comment_flags_comment(comment_id)`, **`UNIQUE idx_comment_flags_comment_user(comment_id, user_id)`**, **`UNIQUE idx_post_likes_post_user(post_id, user_id)`**, `idx_post_views_post_created(post_id, created_at)`, `idx_post_views_post_ip_created(post_id, ip_address, created_at)`, `idx_post_revisions_post_created(post_id, created_at)`, `idx_scheduled_posts_post_status(post_id, status)`, `idx_scheduled_posts_status_publish(status, publish_at)`, **`UNIQUE idx_post_access_post(post_id)`**, `idx_post_purchases_user_post(user_id, post_id)`, `idx_posts_status_updated(status, updated_at)`, `idx_posts_author_created(author_id, created_at)`, `idx_posts_category(category_id)`, `idx_post_series_posts_series_post(series_id, post_id)`, **`UNIQUE idx_post_series_slug(slug)`**. |
| **8** | **One publish path, or a gate on both**                 | Product decision, but the code change is forced either way: make `POST /post/v1/schedule` set `posts.published_at` (and a cancel that NULLs it) and delete `scheduled_posts`/`due_pending`, **or** make the scheduler read `scheduled_posts` instead of `posts.published_at`. Until then, any future review gate must be applied in `services/scheduler.rs:31-36` as well as `post_v1/controller.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

### P1 — next

| #   | Change                                                                                                                                                                                           | Why P1                                                                                                                                                                                                                                          |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 9   | **Delta sync**: client sends `since` (its last successful watermark); server returns rows with `updated_at > since` plus a `has_more` cursor; add `(user_id, updated_at)` indices on both tables | Removes the 3.4 GB/day mobile-data cost for legitimate heavy users. Needs wire + client changes (`schema.ts`, `domain.ts`, `engine.svelte.ts`), so it is not P0; the caps in #1 already kill the abuse.                                         |
| 10  | **Comment evidence capture** (the `E1` §3 `case_evidence` design): before any comment update/delete with `flags_count > 0`, copy the row + its flags into the evidence store                     | Pairs with #5; needs the `E1` table, which is its own workstream.                                                                                                                                                                               |
| 11  | **Per-account flag limiter + self-flag handling**                                                                                                                                                | `flag`: `abuse_limiter` on `comment:flag:user:{id}` (20/h, 50/day, no long tier); skip or count-separately a flag where `comment.user_id == user.id`.                                                                                           |
| 12  | **Post per-account caps + write budget**                                                                                                                                                         | Needs a product decision on what an Author may publish; the role is admin-assigned, so this is a rogue-Author control, not a signup control. Minimum: cap total posts/account and add a daily content-byte budget; alarm on DB growth.          |
| 13  | **Validate verses against real counts**                                                                                                                                                          | Replace the global `ayah ≤ 286` with a 114-entry per-surah count table so impossible verses cannot be stored (the 6,236-verse bound becomes real).                                                                                              |
| 14  | **DB-backed view dedup**                                                                                                                                                                         | Call the existing `post_view::has_viewed_recently` (`actions.rs:79-97`) instead of / in addition to the in-memory `dedup_nx`, and cap `post_views` retention (this is `B8`/`E1`'s sweeper; it must ship with the legal-hold probe per `E1` F7). |

---

## 9. Attack the fix set — what legitimate behaviour breaks

1. **The naive folder cap breaks renames.** Writing the cap as
   `INSERT INTO bookmark_folders … SELECT … WHERE (SELECT COUNT(*) …) < 500 ON CONFLICT (id) DO UPDATE`
   blocks the _update_ path too: once the account is at the cap, no existing folder can be renamed,
   because the SELECT yields no row and the conflict clause never runs. The cap must be checked only
   for **new ids** (pre-check `owned_by_user`), with updates exempt. This is the single most likely way
   to ship a broken fix here.
2. **The folder cap is the user-visible one.** 500 folders is generous against a 114-surah corpus, but a
   madrasa teacher with a folder per student/class can hit it. Mitigations: make it config, return a
   specific error (not a silent skip), and have the client drop the optimistic overlay and tell the user.
   Without the client half, `store.svelte.ts:490-495` keeps the pending folder in `#pendingFoldersById`
   forever → the user sees a folder that never syncs and never disappears. That is worse than an error.
3. **The `updated_at` clamp changes LWW outcomes.** A legitimate offline edit queued for 3 days is in the
   past → unaffected. A device with a fast clock now has its edits clamped to server-now, which can make
   them _newer_ than they were relative to another device's edit — i.e. the clamp can flip a winner. That
   is the intended repair for a wrong clock, but it means the fix is only safe because the window (24 h)
   is far outside normal NTP drift. Reject (not clamp) beyond 365 days so a hostile value fails loudly.
4. **Refusing edits on any flag creates a freeze weapon.** With `flags_count >= 1`, one account (or the
   author's own self-flag) could freeze a target's comment. Hence the threshold of 3 and the flag limiter
   (#11) in the same release. Residual: a coordinated 3-account flag still freezes a comment until an
   admin clears it — acceptable, because the alternative is that flagged evidence is editable.
5. **The `flags_cleared_at` marker needs the update path fixed too.** `comment_flag::create`'s
   update-existing branch (`actions.rs:22-25`) rewrites `reason` but leaves `created_at`, so a pre-clear
   flagger who re-flags stays invisible to the new count. Either bump `created_at` there or add
   `updated_at` and compare on it. Miss this and the clear looks durable while a real re-flag is dropped.
6. **`UNIQUE` indices can fail on existing data.** `post_likes` and `comment_flags` may already contain
   duplicates (the app-level check is only atomic because the pool is 1; the seed system writes rows
   directly). The migration must dedupe before creating each UNIQUE index — `m000008` already establishes
   the pattern (normalize → dedupe → unique).
7. **The per-account sync limiter can throttle three tabs.** Each tab runs its own `SyncEngine` over the
   shared IndexedDB (`outbox.ts` has no cross-tab lease; `take` does not claim rows), so N tabs ≈ N× the
   rounds. With the 2 s debounce + 30 s periodic + visibility flushes, one active tab peaks near 30/min;
   three tabs exceed 60/min. Either set the cap at 120/min or count **mutations** (e.g. 2,000/hour/account)
   instead of rounds. Do not set it below the legitimate multi-tab ceiling.
8. **`truncated` without client handling is data loss.** `applyServer` replaces `#folders`/`#bookmarks`
   with the snapshot (`store.svelte.ts:337-344`), so a partial snapshot silently deletes every local row
   past the cut. Any truncation must be paired with a client refusal path.
9. **Delta sync (`since`) interacts with deletes.** With no tombstones, a delta stream cannot express
   "this bookmark was deleted on another device" — the client would keep it until a full pull. Delta sync
   therefore needs either tombstones (`deleted_at`) or a periodic full reconciliation. This is why #9 is
   P1 and why the caps (#1) are the P0 fix.
10. **Unifying the publish path removes an implicit cancel.** Today an Author can cancel by clearing
    `published_at`; if `/schedule` becomes the only writer, it needs an explicit cancel that NULLs the
    column, or drafts with a past `published_at` will keep auto-publishing after an apparent cancel.

---

## 10. Acceptable to defer, and why

| Item                                             | Defer reason                                                                                                                                                         | Un-defer trigger                                                            |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Delta/cursor sync (#9)                           | The caps make the snapshot bounded by construction (~2 MB worst case); delta is a UX/data-cost optimisation, and it needs tombstones or reconciliation to be correct | A user with > 2,000 bookmarks, or measured p95 snapshot > 256 KB            |
| Comment revision history (#10)                   | Needs a table; `E1`'s `case_evidence` is the designed home and is not built                                                                                          | The first content dispute, or the first `admin_delete` on a flagged comment |
| Server-side versioning / CRDT for bookmarks      | Single-user multi-device bookmark sets have no concurrent-collaboration requirement; LWW is sufficient once skew is clamped                                          | A shared-account feature, or measured "lost edit" complaints                |
| Post per-account caps (#12)                      | The role is admin-assigned, so this is a rogue/compromised-Author control, not an admission-control one; the honest P0 is the DB-growth alarm                        | The first self-serve Author onboarding, or the first storage incident       |
| `scheduled_posts` table removal                  | Dead table; dropping it needs a migration and the publish-path decision is a product call                                                                            | The publish-path unification (#8)                                           |
| Like/view anti-inflation beyond the unique index | View counts are vanity; the real cost is unbounded `post_views` PII, which is `B8`/`E1` retention work and must ship with the legal-hold probe                       | The retention sweeper ships (`E1` F7 sequencing)                            |
| Per-surah verse validation (#13)                 | Impossible verses are invisible storage, bounded at 32,604 rows; the folder cap is the urgent bound                                                                  | The bookmark cap work lands (one small table, same PR if convenient)        |

---

## 11. Findings

All **NEW** unless marked otherwise. Severity is for this surface (sync + UGC lifecycle), not app-wide.

**G2-F1 — Bookmark folder upsert has no per-user cap: 1.2M rows/hour and ~340 MB/hour from one verified
account on one IPv4. NEW. High.**
Evidence: `bookmark_v1/validator.rs:63-65` (≤200 mutations/round); `bookmark_folder/actions.rs:31-61`
(bare `INSERT … ON CONFLICT` with no COUNT guard); `router.rs:174-177` (100/min/IP);
`crates/rux-request-gate/src/layer.rs:239-241,278,378-381` (bucket = per-IPv4 `/32` + matched route);
`auth_guard.rs:57-70` (`verified` is the only gate). `F6` §2.2 stated the vector; the row/byte math in
§1.2 is new.
Fix: §8 #1 (+ #2, #4).

**G2-F2 — The sync snapshot is unbounded and re-sent every round: ~216 MB after one hour, ~660 GB of
response bytes in the first hour, ~0.5–0.8 GB peak per request. NEW. High.**
Evidence: `bookmark_v1/controller.rs:103-120` ("unbounded by design" comment + both `list_for_user`
reads); `bookmark_folder/actions.rs:104-111`; `bookmark/actions.rs:187-194`; the scan + sort happens
inside the transaction (`controller.rs:149-157`) while the response tree is built after commit
(`:159-166`).
Fix: §8 #1, #3; delta as §8 #9.

**G2-F3 — The only limit on bookmark sync is per-IP; there is no per-account throttle, and the same
endpoint is the legitimate heavy user's ~3.4 GB/day mobile bill. NEW. Medium-High.**
Evidence: `router.rs:174-177`; `layer.rs:378-381`; `engine.svelte.ts:22-28,88-90,210-213` (30 s periodic +
2 s debounce + visibility flush); `store.svelte.ts` (each round applies the full snapshot).
Fix: §8 #4, #9.

**G2-F4 — `updated_at` is fully client-supplied and unclamped: a forward-dated row locks the account's
other devices, permanently for year-9999 input. NEW. Medium.**
Evidence: `bookmark_v1/validator.rs:57`; `bookmark_folder/actions.rs:47-49`; `bookmark/actions.rs:66-68,83-84,144-145,178-179`
(delete is LWW too, so the row cannot even be removed).
Fix: §8 #2.

**G2-F5 — Conflict resolution is LWW with no server versioning and no tombstones; an offline edit is
silently dropped, and duplicate folders with the same name are allowed. NEW. Medium.**
Evidence: `bookmark/actions.rs:11-46` (LWW doc), `:99-128` (verse collapse only);
`store.svelte.ts:313-345` (snapshot replaces local state); `engine.svelte.ts:322-347` (`skipped` is
counted and discarded); `domain.ts:69-73`.
Fix: §8 #2 + surface `skipped` in the client; defer CRDT (§10).

**G2-F6 — `expectedUserId` is a client account-change detector, not an authorization boundary; it is
optional, and it is on every mutation only because `/sync` is the module's sole route. NEW (corrects the
framing in `B2` F10).**
Evidence: `bookmark_v1/controller.rs:131-136`; `bookmark_v1/mod.rs:8-11`; `domain.ts:51-64` (client
string-matches the 403). Cross-user safety comes from `bookmark_folder/actions.rs:44-49`,
`bookmark/actions.rs:61-97,136-145`, `controller.rs:38-49`, pinned by
`tests/bookmarks_v1.rs::cross_user_ids_are_isolated`. No corruption path found.
Fix: none required; state the invariant in the sync workstream so the field is never mistaken for a guard.

**G2-F7 — A comment can be edited after being flagged or hidden: the reported text is overwritten with
no revision history. NEW. High.**
Evidence: `post_comment/actions.rs:48-82` (no `hidden`/`flags_count` check);
`post_comment_v1/controller.rs:95-125` (no limiter either); `m000001_init.rs:283-297` (no revision
table for comments).
Fix: §8 #5, with the freeze-threshold caveat in §9.4.

**G2-F8 — `admin_flags_clear` is silently reverted: the next flag recounts all surviving rows and the
count jumps back. NEW. High.**
Evidence: `post_comment/actions.rs:389-400` (count = 0, rows kept);
`comment_flag/actions.rs:13-40` (update-existing + `sync_flags_count`), `:162-177` (recount from rows);
`comment_flag/actions.rs:179-186` (dead `clear_flags`, still zero callers).
`E1` F1/`F3` F3 noted the count signal is destroyed; the resurrection by the next flag is not in any
prior report.
Fix: §8 #6 (+ §9.5 for the update-path trap).

**G2-F9 — Flag has no per-account limiter and self-flagging is allowed: one account can flag every
comment at 100/min/IP (queue flooding), and delete+repost resets `flags_count` to 0. NEW. Medium-High.**
Evidence: `post_comment_v1/controller.rs:341-385` (no limiter, no self-flag check);
`post_comment_v1/mod.rs:8-16` (`verified` only); `router.rs:120-123`; `post_comment/actions.rs:85-105`
(owner delete) + `m000001_init.rs:304-305` (flag CASCADE); `post_comment/actions.rs:285-297`
(`flags_count` is the queue's ordering signal).
Fix: §8 #11 (+ #6).

**G2-F10 — `post_like` has no `UNIQUE(post_id, user_id)`; `post_view` dedup is in-memory, fails open, and
the DB-backed dedup is dead code; one IP can insert 288k view rows/day. NEW. Medium.**
Evidence: `m000001_init.rs:185-194` (no unique); `post_like/actions.rs:20-83` (app-level check);
`post_v1/controller.rs:346-386` (public route + `dedup_nx`); `crates/rux-request-gate/src/abuse.rs:127-138`
(fail-open, tested `:205-207`); `post_view/actions.rs:79-97` (`has_viewed_recently`, no callers);
`post/actions.rs:616-623` (raw IP + UA per row).
Fix: §8 #7, #14.

**G2-F11 — Post writes are uncapped per account and the three write routes hold independent 200/min
buckets: ~9 GiB/hour of post+revision content; revisions are capped at 10/post but new posts are not.
NEW. Medium-High.**
Evidence: `post_v1/mod.rs:40-50` (three routes, one POST body limit) + `router.rs:115-118` +
`layer.rs:278` (per-matched-route buckets); `post_v1/validator.rs:260-277` (payload);
`post_revision/actions.rs:13,35` (10/post, enforced in-tx); `config/mod.rs:11` (256 KiB).
Fix: §8 #12, #7 (`idx_posts_*`), and a DB-growth alarm.

**G2-F12 — There is no publish-time moderation, and there are two publish paths: the controller's
`is_published`/`status` and the scheduler's `posts.published_at` tick, while `/post/v1/schedule` writes a
table nothing reads. NEW. High.**
Evidence: `services/scheduler.rs:31-36,70-80` (draft + `published_at <= now` → Published, no moderation);
`post_v1/controller.rs:599-615` (schedule → `scheduled_posts`); `scheduled_post/actions.rs:85-102`
(`due_pending` has no callers — grep); `post_v1/validator.rs:266,286-290,308,316-322`;
`post_v1/controller.rs:156-185,251-286`.
Fix: §8 #8.

**G2-F13 — Ten UGC tables have zero indices and three lack integrity uniques; every list/queue query is a
full scan on a pool-of-1 SQLite connection. NEW. Medium-High.**
Evidence: `m000001_init.rs` has no `CREATE INDEX`; the full index inventory is `m000004`–`m000011`
(grep) and covers only bookmarks/devices/notifications/oauth/email-codes/passkeys. Missing set in §7.
`F6` §4 said "several missing indices" from an earlier audit; this enumerates the UGC set.
Fix: §8 #7.

_(12 findings as requested; F6 and F13 are the two that split a prior report's claim from its
enumerated evidence.)_

---

## 12. Scope note and residual

Audited: bookmark sync (push/pull, caps, snapshot, LWW, guard, client engine/outbox/conflict apply),
post lifecycle (create/update/autosave/revisions/schedule/series/publish/paywall read), comment
lifecycle (create/edit/delete/flag/admin actions), like/view, body caps, indices, and the migrations for
those tables.

Not audited here (other reports own them): media pipeline and variants (`F6` §2.1), the `audit_logs`
writer design (`F3` §4), the retention sweeper and legal holds (`E1` §3/§5, `B8`), the admin destruction
chain (`F3` §2), notification fan-out caps (`D5` item 3), and the reader-path rate limits (`B3`/`E6`).
The honest residual for this report: I read every file listed in §0 but did not execute the sync endpoint
against a live DB, so the byte/row figures are computed from the schema and the validators, not measured.
The 32,604-row bookmark bound and the 200/round × 100/min folder rate are code facts; the per-row byte
sizes are estimates (±30%) and are stated as ranges.
