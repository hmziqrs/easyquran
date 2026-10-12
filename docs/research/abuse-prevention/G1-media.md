# G1 — Media pipeline audit, end to end (round 8)

Auditor: media-pipeline subagent. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` (working tree,
read-only). Inputs read first: `F6-remaining-gaps.md` §2.1 (hole 1), `E1-incident-response.md` (§0 ground
truth, §3 preserve-first, §4 act), `D5-final-gaps.md` #6/#9, `E5-fresh-eyes.md` (F7, F9), `A7-performance-cost.md`
(pool-of-1, fsync, per-request budget). Then the code, end to end, this session. Every `file:line` below was
read in the tree. No repo writes, no DB writes, no external system contacted.

**Method note.** I re-derived every F6 §2.1 claim from source instead of trusting it, and extended the read
to the parts F6 did not open: the moderation wiring and its ordering, the client side, the storage/URL layer,
the FK graph in both directions, the user-erasure path, and the operator's actual takedown levers. Where a
claim needs a live system to settle (CDN TTL, R2 domain binding), it is marked **unverifiable from the repo**
rather than asserted.

**Reach caveat that bounds every finding below.** The pipeline is _mounted and reachable_ but _latent_:
`media` has **0 rows** in the dev DB (`sqlite3 file:rust/data/easyquran.db?immutable=1 "select count(*) from
media"` → 0, same for `media_variants`, `media_usage`), there is **no upload UI and no client at all** in
`web/` (`grep -rn "FormData|multipart|media/v1" web/src` → 0 hits outside tests; no `lib/media*`, no upload
route), the only gate is `verified + ROLE_AUTHOR` and Author is admin-assigned (`media_v1/mod.rs:19-21`;
`user_v1/mod.rs:34-36`), and `media`/`media_variants`/`image_optimizer` appear in **no repo doc** (`grep -ri
media docs/` → 0 hits; `docs/security.md` has only the R2-06 post-attachment finding). So today the abuser
set is "the operator plus any admin-granted author". The moment an upload client exists, or a second Author
is granted, or a takedown is attempted, every finding below is live — and finding 1 is live at the first
takedown regardless of who uploaded.

---

## 0. Pipeline table (stage → enforcement point → exact limit → gap)

| #   | Stage                 | Code                                                                               | Enforcement point                                                                                                | Exact limit                                                                                                                                                                   | Gap                                                                                                                                                                                                |
| --- | --------------------- | ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Transport / auth      | `media_v1/mod.rs:8-22`, `router.rs:129-132`                                        | `DefaultBodyLimit` + nest rate limit + `auth_guard::verified_with_role::<ROLE_AUTHOR>`                           | **2 MiB** request body; **30 req/60 s per IP per matched route**; session + CSRF + verified + role ≥ Author + not-banned                                                      | Per-IP only, no per-account daily cap; IP is `CF-Connecting-IP` (spoofable per E5 F1)                                                                                                              |
| 2   | Multipart parse       | `controller.rs:182-227`                                                            | handler loop; `file` field buffered whole (`field.bytes()`)                                                      | file field ≤ 2 MiB (re-checked `:199-211`)                                                                                                                                    | unknown metadata fields silently ignored (`services/media.rs:47`); no field-count/name cap                                                                                                         |
| 3   | Type allowlist        | `validator.rs:8-17,46-88`; `controller.rs:236-240`                                 | **declared** `Content-Type` first, **filename extension** as fallback                                            | `image/{jpeg,jpg,png,webp,gif,tiff}` / `.jpg .jpeg .png .webp .gif .tiff`                                                                                                     | **bytes are never sniffed**; declared MIME wins over the real format (see finding 2)                                                                                                               |
| 4   | Dedup                 | `controller.rs:242-266`; `media/actions.rs:67-79`                                  | SHA-256 of bytes, global lookup                                                                                  | —                                                                                                                                                                             | unindexed scan (no index in `m000001_init.rs`); **runs before moderation** (finding 3); cross-user hit falls through to a fresh record (no key leak — verified)                                    |
| 5   | Moderation            | `controller.rs:272-305`; `main.rs:495-522`                                         | `if let Some(moderator)`                                                                                         | provider-dependent; **enabled flag default `false`**; missing URL → `None` + warn                                                                                             | **no moderator at all by default**; fail-closed only on _provider error_, fail-open on _config error_ (finding 3)                                                                                  |
| 6   | Dimensions (metadata) | `controller.rs:307-321`                                                            | `imagesize::blob_size` best-effort `if let Ok`                                                                   | —                                                                                                                                                                             | result is metadata only; never used as a gate                                                                                                                                                      |
| 7   | Optimize / decode     | `controller.rs:344-402`; `image_optimizer.rs:121-300`                              | `spawn_blocking` → `probe()` (header) → pixel budget → bpp skip → `load_from_memory`                             | `OPTIMIZE_ON_UPLOAD=true`, `OPTIMIZER_MAX_PIXELS=12_000_000`, `OPTIMIZER_KEEP_ORIGINAL=true`, quality 80 (`settings.rs:170-179`)                                              | pixel budget checked **before** decode (good) but 12 M px ≈ 48 MB RGBA per decode, `spawn_blocking` unbounded → finding 11; any skip/failure keeps the original bytes verbatim                     |
| 8   | Original write        | `controller.rs:409-431`                                                            | `put_object(bucket, key, body, content_type)`                                                                    | key = `media/{YYYY}/{MM}/{uuid4}.{ext}` (`:881-890`)                                                                                                                          | **object written before the DB row**; no `Cache-Control`, no `Content-Disposition`, no lifecycle tag (finding 6/9)                                                                                 |
| 9   | Variant write         | `controller.rs:433-487`                                                            | one `put_object` per variant                                                                                     | keys `{base}@{width}w.{ext}` / `{base}@lqip.{ext}`                                                                                                                            | **errors logged and ignored** (`:471-486`); rows pushed before the PUT (`:456-469`)                                                                                                                |
| 10  | DB rows               | `controller.rs:489-528`; `media/actions.rs:17-56`; `media_variant/actions.rs:8-38` | 1 insert + N inserts, **no transaction**                                                                         | —                                                                                                                                                                             | rows ↔ objects not atomic in either direction (finding 9)                                                                                                                                          |
| 11  | Response              | `controller.rs:530-536`; `media/url.rs:3-11`                                       | `build_public_file_url(public_url, bucket, key)`                                                                 | `https://r2.easyquran.fyi/{bucket}/{key}`                                                                                                                                     | only the **original** URL is ever returned — variant URLs are derivable, never handed out (finding 1); bucket-prefix shape contradicts the quran path (finding 10)                                 |
| 12  | List / view / usage   | `controller.rs:550-801`                                                            | `can_view_media` (owner or role ≥ Moderator); non-privileged list forced to `caller.id` (`validator.rs:129-151`) | 20/page, unbounded `page` (`media/actions.rs:216-219`)                                                                                                                        | no cross-user read found (verified: `view`, `list/query`, `usage/details` all scoped) — but a Moderator reads **everything** (finding 5)                                                           |
| 13  | Delete                | `controller.rs:803-860`                                                            | `can_delete_media` (owner or role ≥ Moderator) → **one** `delete_object` → row delete                            | 1 object                                                                                                                                                                      | **variants survive** (finding 1); no audit, no tombstone, no bulk, no purge                                                                                                                        |
| 14  | FK aftermath          | `m000001_init.rs:29-47,195-220`                                                    | SQLite FKs                                                                                                       | `media_variants.media_id` **CASCADE**; `media_usage.media_id` **CASCADE**; `posts.featured_image_id`, `categories.cover_id/logo_id` **SET NULL**; `users.avatar_id` **no FK** | variant key rows destroyed with the media row (that is why the objects are unrecoverable from the app); `avatar_id` dangles; **user delete only NULLs `media.uploader_id`** → finding 8            |
| 15  | Serving               | `docker-compose.yml:148-149`; `.env.example:20`; `media/url.rs:3-11`               | R2 public custom domain, direct                                                                                  | `S3_PUBLIC_URL` default `https://r2.easyquran.fyi`; bucket `S3_BUCKET` (default `easyquran-assets`, `.env.example` says `easyquran`)                                          | app has **no revocation primitive** — no proxy, no signed URL, no deny-list, no purge (findings 1/6); web SW ignores the host (`web/src/service-worker.ts:620`) so the browser fetches R2 directly |
| 16  | Client                | `web/src/**`                                                                       | **none**                                                                                                         | —                                                                                                                                                                             | no upload path, no chunking, no retry, no client-side validation, no UI: `grep -rn "FormData\|multipart" web/src` → 0 hits                                                                         |

Two independent facts follow from the table and frame the whole audit:

- **The app cannot revoke.** Media is served straight off a public bucket; nothing in `rust/backend/api/src`
  or `web/src` can make a stored object unreachable. The only levers are the S3 API (delete) and Cloudflare
  (purge) — and the code uses exactly one of them, on exactly one key.
- **The app cannot verify storage state.** There is no `list_objects_v2` call anywhere
  (`grep -rn "list_objects" rust/backend/api/src` → 0), so orphan objects in either direction are invisible
  to every code path and to the operator.

---

## 1. Upload path — what is actually validated

**Sequence** (`controller.rs:163-537`): auth → multipart → declared-MIME/extension allowlist → SHA-256 dedup
→ moderation (maybe) → `imagesize` probe → optimizer → `put_object` original → `put_object` variants → DB rows.

**What is enforced, exactly:**

1. **Size: 2 MiB** (`config/mod.rs:12`), applied as `DefaultBodyLimit::max(config::body_limits::MEDIA)` on
   the `/create` route (`media_v1/mod.rs:9-11`) — axum's `Multipart` wraps the body in `with_limited_body()`
   (`axum-0.8.9/src/extract/multipart.rs:72-74`), so the whole multipart body, not just the file field, is
   capped — plus a handler re-check on the file field (`controller.rs:199-211`). Machine-tested
   (`tests/request_body_limits.rs:149-198`, fixtures `2mbunder.jpg` / `2mbplus.jpg`).
2. **Type: declared header, then filename.** `validate_upload` (`validator.rs:46-88`) accepts if the
   declared MIME is allowlisted; otherwise it falls back to the filename extension and _synthesises_ the MIME
   (`mime_for_extension`). If the declared MIME is allowlisted but the filename extension is not, the
   extension is replaced with one derived from the MIME (`:80-85`) — pinned by
   `validate_upload_strips_untrusted_extension_but_keeps_allowed_mime`.
3. **Nothing else.** No magic bytes, no decode-before-store requirement, no dimension gate, no per-user
   accounting. The only byte-level inspection is `imagesize::blob_size` for _metadata_ (`controller.rs:307-321`,
   `if let Ok`), and the optimizer's `image::guess_format` inside `probe()` (`image_optimizer.rs:308`) — which
   is used to pick a format _hint_, not to reject.

**Answers to the specific questions:**

- **Decompression bomb?** Not a server-side bomb, by construction: `probe()` is header-only and the pixel
  budget (`image_optimizer.rs:171-182`, 12 M px) is checked _before_ `load_from_memory` (`:199`). A file whose
  header declares more than 12 M px is skipped and **stored verbatim** (2 MiB) — the API never decodes it.
  The residual is not a bomb but a _decode-cost_ problem: ≤12 M px still means ~48 MB RGBA + resize/encode
  buffers per upload, with no concurrency cap (finding 11).
- **SVG with script?** Rejected by allowlist (`validator.rs:295-297`, `:321-326`, `:336-346`), and the
  optimizer's output MIME is re-checked (`controller.rs:379-388`). But: SVG _content_ declared as
  `image/png` is accepted and stored with `Content-Type: image/png` (see next bullet) — inert in a browser
  as long as R2 serves the stored type, but it is an arbitrary-byte container on the brand domain.
- **Polyglot?** Accepted and stored **verbatim** with the default config: the original bytes are only replaced
  when the optimizer re-encodes, and that path is off by default (`OPTIMIZER_KEEP_ORIGINAL=true`,
  `settings.rs:175`; the gate is `image_optimizer.rs:236-243`). A valid JPEG with an appended ZIP/HTML/ELF
  payload is a valid JPEG to every check here.
- **Non-image?** **Accepted.** `validate_upload(Some("image/png"), Some("x.png"))` passes on the declared
  header alone; `imagesize::blob_size` fails (metadata stays `None`); `probe()` fails
  (`UnsupportedFormat`/`DecodeFailed`) or `load_from_memory` fails (`DecodeFailed`) → `OptimizationOutcome::Skipped`
  → `final_bytes = file_bytes.clone()` (`controller.rs:325`, `:364-376`) → the bytes are PUT to R2 with
  `content_type = "image/png"` and key `media/2026/10/<uuid>.png`. So any ≤2 MiB payload becomes a publicly
  served object on `r2.easyquran.fyi` under an image MIME. The declared MIME is also what the optimizer's
  `probe()` reports as `probed.mime` (the hint wins over `guess_format`, `image_optimizer.rs:315-318`), so the
  served `Content-Type` can disagree with the bytes even when the bytes _are_ an image.
- **Format allowlist vs. decoder set.** `image/gif` and `.gif` are allowlisted (`validator.rs:13,17`) but the
  `image` crate is compiled without the `gif` feature (`Cargo.toml:137`: `["jpeg","png","webp","tiff"]`), so
  **every GIF takes the `DecodeFailed` path**: stored verbatim, never optimized, never metadata-stripped, no
  variants. Same outcome class for any format the decoder set lacks.

**Enforcement point, one line:** `media_v1/controller.rs:236-240` (`validate_upload`) decides _acceptance_;
`config::body_limits::MEDIA` = 2 MiB decides _size_; nothing decides _what the bytes are_.

---

## 2. Storage exhaustion — quota, cap, accounting

- **Per-user quota: none.** `grep -rni "quota" rust/backend/api/src` → one unrelated mail comment. There is
  no count cap, no byte cap, no `SUM(size)` check, no admission control on `/create` beyond auth.
- **`media_usage` is a ledger, not a quota.** It records `(media_id, entity_type, entity_id, field_name)`
  when a media is _attached_ to a post/category/user (`media_usage/actions.rs:12-37`; callers
  `category/actions.rs:119,130`, `user/actions.rs:377`); it is never read on the upload path and never
  compared against a limit. Its only reads are the `usage_count` decoration in list/view
  (`media/actions.rs:226-235,266-270`) and the moderator usage-details endpoint.
- **Rate limit: 30/60 s per IP per matched route** (`router.rs:129-132`). It is per-IP, keyed on the
  `CF-Connecting-IP`-derived identity (`middlewares/rate_limit.rs:22-28`), which E5 F1 shows is forgeable by
  anyone who can reach the origin. There is no per-account bucket.
- **What one account can do.** 30 uploads/min × 60 min × 24 h = **43,200 uploads/day**, each ≤2 MiB of
  request bytes and 1–7 stored objects. At ~2 MiB stored per upload (original + variants) that is
  **~86 GB/day per IP**. Cost on R2 (storage $0.015/GB-month, Class A $4.50/M): ~2.6 TB/month ≈ **$39/mo
  storage + ~$41/mo Class A ≈ $80 per sustained IP-day**, with no cap, no alert, no anomaly metric, and no
  total-bucket ceiling. This is an estimate (variants are usually smaller than the original; the 30/min is a
  per-IP limit that IP rotation defeats).
- **Disk:** none consumed locally — bytes go straight to R2. So the failure mode is _cost + brand-domain
  content_, not a full VPS disk. (Contrast `E5` A3, where unauthenticated `sessions` rows _do_ hit local disk.)
- **No index on `content_hash`** (no `CREATE INDEX` anywhere in `m000001_init.rs`) → the dedup lookup is a
  full scan of `media` on every upload; irrelevant at 0 rows, a real cost at scale.

---

## 3. Variants — every derived object, and the verified defect

**What is produced.** The optimizer returns an `OptimizationResult` whose `variants` are uploaded with the
keys built at `controller.rs:433-449`, using the strategy selected by `reference_type` (default `Post` when
the form field is absent — `image_optimizer.rs:193-197`):

| Strategy                      | Widths (only `<` source width/dimension)                  | Format                     | Keys                                                     | Count |
| ----------------------------- | --------------------------------------------------------- | -------------------------- | -------------------------------------------------------- | ----- |
| `Post` (default)              | 480, 768, 1024, 1600, 2048 (`image_optimizer.rs:496-506`) | PNG if alpha else JPEG q80 | `{base}@480w.jpg`, `@768w`, `@1024w`, `@1600w`, `@2048w` | ≤5    |
| `Post` LQIP                   | 24 (`:508-516`)                                           | JPEG q40                   | `{base}@lqip.jpg`                                        | 1     |
| `User`                        | 48, 96, 192, 384, exact-square crop (`:431-449`)          | PNG if alpha else JPEG q82 | `{base}@{n}w.{ext}`                                      | ≤4    |
| `Category` icon (aspect ≤1.5) | 128, 256 (`:450-465`)                                     | PNG q100                   | `{base}@{n}w.png`                                        | ≤2    |
| `Category` wide               | 640, 1280, 1920 (`:466-484`)                              | PNG if alpha else JPEG q80 | `{base}@{n}w.{ext}`                                      | ≤3    |

`{base}` = `object_key` minus its extension (`controller.rs:410-413`); `{ext}` = `jpg` or `png` from
`encode_to_format` (`image_optimizer.rs:667-713`). **Max 6 variants + 1 original = 7 objects per upload.**

**When variants exist at all.** Only when the optimizer returns `Optimized` — i.e. decode succeeded, pixels
≤12 M, and the bpp skip did not fire (`image_optimizer.rs:171-191`, `:199-209`, `:385-391`; JPEG/WebP skip at
≤1.5 bpp, PNG at ≤3.0). A typical 2 MiB phone JPEG (bpp ≈ 0.2) **skips entirely → 0 variants**. The variant
surface is therefore dominated by small-but-heavy images (screenshots, graphics, near-lossless JPEGs). The
DB rows are the **only** record of the variant keys — nothing writes a manifest, and no code reads
`media_variants` at runtime (only the feature-gated seed module does).

**The defect, verified line by line (F6 §2.1 confirmed and extended):**

1. `controller.rs:832-849` — `delete_object()` on `media.object_key` **only**. `grep -rn "delete_object"
rust/backend/api/src` → exactly one call site, this one.
2. `controller.rs:851` → `media/actions.rs:82-102` — the `media` row is deleted.
3. `m000001_init.rs:195-210` + `media_variant/model.rs:26-33` — `media_variants.media_id` is
   `ON DELETE CASCADE`, so every variant row (the only key record) dies in the same statement.
4. No sweeper: `grep -rn "list_objects" rust/backend/api/src` → 0 hits; no lifecycle rule, no tombstone
   table, no GC job anywhere in the repo or `deploy/`.
5. Public reachability: `media/url.rs:3-11` builds `https://r2.easyquran.fyi/{bucket}/{key}` and the bucket
   is the public custom domain (`docker-compose.yml:148-149`; `deploy/README.md:88` calls it "the public R2
   base"). No auth on objects, no signed URLs.

**What survives a delete, exactly:** for each generated variant, the object at
`https://r2.easyquran.fyi/{bucket}/media/{YYYY}/{MM}/{uuid}@{480|768|1024|1600|2048}w.{jpg|png}` and
`…@lqip.jpg` remains in R2 and is served to anyone with the URL or the ability to derive it (the pattern is
trivial: take the original URL, strip the extension, append the suffix — a handful of guesses, both
extensions). For CSAM/illegal-content takedown that is the difference between "removed" and "still up".

**The fix, ranked by robustness:**

- **(a) Enumerate-and-delete in the delete handler (minimum, must ship):** read the variant keys while the
  rows still exist, delete each object, and only then delete the rows — keeping the rows on any S3 failure
  so the operation is retryable. Variants deleted _before_ the parent row delete must not rely on CASCADE.
- **(b) Tombstone/manifest:** write `(media_id, object_key, content_hash, actor, deleted_at)` to a
  `media_deletions` row in the same transaction _before_ the objects go, so the takedown is auditable and a
  sweeper can retry a partial failure.
- **(c) Orphan sweeper:** `list_objects_v2(prefix="media/")` → delete objects older than N hours whose base
  key matches neither `media.object_key` nor `media_variants.object_key`. This is the only way to clean up
  the orphans this defect has already created, plus upload partial failures (finding 9). Run as a manual
  `just media-sweep` (dry-run first) — the repo has no scheduler slot for it and none should be added
  casually (A7 shows the single-writer budget).
- **(d) A lifecycle rule on the bucket is _not_ a fix** for takedown (it is time-based, not event-based) and
  nothing in the repo can configure one.

---

## 4. Takedown — the exact sequence today, and what it leaves behind

**What the operator actually has:** no admin console (`E1` §0/A8 §9 cut it), no `just incident` recipes
(`grep -n "incident\|media" justfile` → 0), no bulk delete, no delete-by-key endpoint, no purge, no audit
row. The only in-app verb is `POST /media/v1/delete/{media_id}` (`media_v1/mod.rs:17`) behind session + CSRF

- `verified_with_role::<ROLE_AUTHOR>`.

**Sequence, as the code stands (verified):**

1. Find the media id. There is no admin media UI; the operator must list as a Moderator/Admin
   (`POST /media/v1/list/query`, `:590-631`) or query the DB.
2. `POST /media/v1/delete/{id}` with a staff session + CSRF header.
3. Ownership check passes for the owner or **any role ≥ Moderator** (`can_delete_media`, `:543-548`).
4. `delete_object` on the original (`:832-849`). **If this fails → 500 and the row survives** (good: the
   operation is retryable and fail-safe).
5. `Media::delete_by_id` (`:851`) → CASCADE destroys `media_variants` + `media_usage`; `posts.featured_image_id`
   and `categories.cover_id/logo_id` are set NULL; `users.avatar_id` **dangles**.
6. Return `{"message":"Media deleted"}`. **No audit row, no tombstone, no record of what was deleted.**

**What is left behind:**

| Leftover                                                       | Reachable?                                                                    | Recoverable by the app?                                                |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Variant objects (`@480w` … `@lqip`)                            | **Yes — public R2/CDN URL, derivable from the original URL**                  | No: the key rows were destroyed in step 5                              |
| CDN edge copy of the original                                  | Yes until TTL (Cloudflare default; **unverifiable from the repo**)            | No purge capability exists (finding 6)                                 |
| Browser HTTP cache / any client that fetched it                | Yes (the SW deliberately ignores `r2.easyquran.fyi`, `service-worker.ts:620`) | No                                                                     |
| `media_variants` / `media_usage` rows                          | Gone                                                                          | n/a                                                                    |
| `media` row + `content_hash` (the evidence E1 §3 step 6 wants) | Gone — **destroyed by the takedown itself**                                   | No; E1's capture must run _before_ this call and nothing enforces that |
| `users.avatar_id` dangling integer                             | n/a                                                                           | Only by a later UPDATE                                                 |
| Post rows that referenced the image                            | Row survives with `featured_image_id = NULL`                                  | n/a (no record of what was there)                                      |
| Audit trail                                                    | **None** (`audit_logs` has zero writers, E1 F1)                               | n/a                                                                    |

**What can go wrong, concretely:** (i) a partial takedown that looks complete (the original 404s while
variants serve — finding 1); (ii) the takedown destroying the very metadata a case needs (step 5, and no
legal-hold check exists because no hold table exists — E1 §3/F7); (iii) no way to prove the takedown
happened or when; (iv) if the row was already deleted by an earlier attempt, the surviving objects are
**unreachable from the app forever** — only an out-of-band S3 listing can find them, using the same
`R2_ACCESS_KEY_ID`/`R2_SECRET_ACCESS_KEY` that sit in `.env` (`.env.example:18-19`) with no documented
runbook.

**The honest current "hurry" path** is therefore out-of-band: use any S3 client against
`https://{account}.r2.cloudflarestorage.com` with the repo's credentials, `delete-objects` on the prefix
`media/{YYYY}/{MM}/{uuid}` (the base key, which removes nothing extra — R2 has no prefix-delete on the S3
API), i.e. enumerate the known suffixes, then optionally call the app delete to clean the row. That is
undocumented, unlogged, and easy to get wrong under pressure — which is exactly the gap `E1` §11 wants
closed with a `just incident` recipe.

---

## 5. EXIF / metadata

**Is EXIF stripped by default? No.** The default configuration keeps the original bytes verbatim:

- `settings.rs:170-179`: `OPTIMIZE_ON_UPLOAD=true`, `OPTIMIZER_MAX_PIXELS=12_000_000`,
  **`OPTIMIZER_KEEP_ORIGINAL=true`**, `OPTIMIZER_WEBP_QUALITY_DEFAULT=80`. Neither `.env.example` nor
  `docker-compose.yml` sets any of them (`grep -n "OPTIMIZER" .env.example docker-compose.yml` → 0), so the
  defaults are what production runs.
- The only metadata-stripping path is `reencode_original` (`image_optimizer.rs:523-572`), gated on
  `!config.keep_original` (`:236-243`). With the default it never runs.
- Even with `keep_original=false` it is conditional: alpha images return `None` (`:532-535,539-541,545-547`),
  category icons return `None`, and a re-encode that is not ≥5% smaller is discarded
  (`significant_reduction`, `:554-556,715-722`) — and the already-optimized bpp skip (`:385-391`) returns
  before the re-encode is even considered. So even in the non-default configuration, a large share of images
  keeps its metadata.
- Any optimizer skip — decode failure, unsupported format (all GIFs), pixel-budget overflow
  (`:153-182,199-209`) — also stores the original verbatim.
- Variants carry no metadata (they are re-encoded from decoded pixels), but they are the _small_ copies; the
  original is the one a viewer opens and the one that carries GPS.

**What leaks:** GPS coordinates, device make/model/serial-adjacent tags, capture timestamps, thumbnails, and
any XMP/IPTC text — in the world-readable original object. Author-only today (finding-reach caveat), but the
privacy policy says nothing about image metadata (`A5` §0), and the object is public the moment the image is
attached to a published post or a profile.

**One more metadata defect (minor, verified):** EXIF **orientation is not applied** on decode —
`image 0.25.10` exposes `ImageDecoder::orientation()` (`src/codecs/jpeg/decoder.rs:140`) and a manual
`DynamicImage::apply_orientation` (`src/images/dynimage.rs:1161`), and nothing in `image_optimizer.rs` calls
either. So any re-encoded original/variant of an orientation-tagged photo can be rotated 90°/mirrored
relative to what every browser shows for the kept original. Latent (nothing consumes variants today).

**Fix:** decode and re-encode the _original_ unconditionally for JPEG/PNG/WebP (strip metadata; the 2 MiB cap
bounds the cost), or add a metadata-stripping pass on the keep-original path (lossless: drop JPEG APPn /
PNG ancillary chunks / WebP EXIF-XMP chunks). Also call `apply_orientation` before re-encoding. And record
the decision in the docs — right now there is no documented media behavior at all.

---

## 6. Moderation — the honest answer

**Uploads are not moderated today.** The chain:

- `main.rs:499` — `let enabled = env_bool("IMAGE_MODERATION_ENABLED", false);` → default **false**.
- `main.rs:502-521` — `enabled` + non-empty `IMAGE_MODERATION_URL` → `HttpModerator`; `enabled` + missing URL
  → `warn!("…moderation disabled and uploads will be accepted unmoderated")` + `None` (**fail-open on
  misconfiguration**); not enabled → `None`.
- `controller.rs:272` — `if let Some(moderator) = state.storage.image_moderator.as_ref()` → with `None` the
  entire block is skipped. There is no "moderation required" policy, no boot refusal, no admin warning
  beyond one log line.
- `NoOpModerator` (`image_moderation/mod.rs:34-49`) is **dead code** — constructed only in its own test
  (`:117-124`); `grep -rn NoOpModerator rust/backend/api/src` → definition + tests only.
- Neither `.env.example` nor `docker-compose.yml` mentions `IMAGE_MODERATION_*` → production runs with no
  provider.

**When enabled, what happens:** `HttpModerator::classify` (`image_moderation/mod.rs:78-110`) POSTs the raw
bytes with `Authorization: Bearer …` and the **declared** `Content-Type` to `IMAGE_MODERATION_URL` and parses
`{safe, scores}`. Provider error (transport, non-2xx, malformed JSON) → `Err` → the controller fails
**closed** with 503 (`controller.rs:294-304`) — that part is right and is pinned by the code comment at
`:270-271`.

**Can an attacker bypass it?** Yes, in three ways, and the first is unconditional:

1. **It is off.** The default is no moderation at all.
2. **Config fail-open.** `IMAGE_MODERATION_ENABLED=true` with a missing/empty URL logs a warning and accepts
   unmoderated uploads. The operator's mental model ("moderation is on") and the runtime state diverge with
   one missing env var.
3. **Ordering/typing holes when it _is_ on:** the dedup short-circuit runs **before** moderation
   (`controller.rs:249-259` returns the existing record for the same uploader), so re-uploading previously
   accepted bytes is never re-classified (matters when moderation is enabled _after_ files exist). And the
   provider sees the **declared** MIME with **unvalidated bytes**: a non-image payload declared `image/png`
   reaches the classifier as an image it cannot parse — whether that is "safe" or an error is
   provider-dependent, so the honest statement is "either the provider rejects it (fail-closed) or the model
   cannot see it as an image". Neither the handler nor the provider enforces "this is a decodable image".

**Verdict for the plan:** the CSAM/image-moderation capability the plan promises does not exist in the
shipped configuration. `E1` F5's "not a NoOp moderator — no moderator at all" is confirmed, and the failure
mode is worse than a NoOp in one respect: a misconfigured enable is indistinguishable from a working one in
the logs' severity (`warn!`), while uploads proceed.

---

## 7. Authorization — who can do what, and the blast radius

| Verb          | Route                          | Gate                                                                                                        | Cross-user?                    |
| ------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Upload        | `POST /media/v1/create`        | `verified + role ≥ Author + not_banned` (`media_v1/mod.rs:19-21`, `auth_guard.rs:72-89`)                    | n/a                            |
| View one      | `POST /media/v1/view/{id}`     | `can_view_media` = owner **or role ≥ Moderator** (`:539-541`)                                               | Moderator reads all            |
| List          | `POST /media/v1/list/query`    | non-privileged forced to `uploader_id = caller.id` (`validator.rs:129-151`); role ≥ Moderator lists **all** | Moderator lists all            |
| Usage details | `POST /media/v1/usage/details` | `media_records` filtered to owner or Moderator before output (`:655-663,736-739`)                           | no leak found                  |
| Delete        | `POST /media/v1/delete/{id}`   | `can_delete_media` = owner **or role ≥ Moderator** (`:543-548`)                                             | **Moderator deletes anything** |

**IDOR verdict (verified, negative for regular users):** a plain Author cannot view, list, or delete another
user's media. `view`/`list`/`usage` are owner-scoped; the `usage/details` endpoint fetches usage rows for any
requested id but drops them unless the media is owned/moderated (`:736-739`), and unauthorized ids are
indistinguishable from nonexistent ones (no existence oracle). Post attachment has its own ownership check
(`post_v1/controller.rs:46-63`, R2-06). **No IDOR found.**

**Round 8's claim verified — and its blast radius:** `can_delete_media` grants delete to `is_moderator()`,
which is `role ≥ Moderator` (`db/sea_models/user/model.rs:49-51`). Pinned by tests
(`controller.rs:978-984`, `:986-995`: `admin_can_delete_null_owner_media`,
`only_owner_or_staff_deletes_owned_media`). Blast radius:

- A **Moderator** — the lowest staff role, and exactly the role `E1` §2/§9 proposes to hand to a backup
  triager — can delete **every media row in the system**: other authors' uploads, category covers/logos
  (`uploader_id NULL` → "Only staff may delete system media", `:822-829`), and any image referenced by
  published posts, one id at a time, **with no audit row, no soft-delete, and no undo**, and (finding 1)
  leaving the variants publicly served. `E1` §9's mitigation ("the backup must only be able to hide content")
  is not what the code does.
- The same role reads every media row and URL (unpublished images, other users' avatars, object keys,
  content hashes) via `list/query`/`view`/`usage/details`.
- There is no admin-only variant of delete, no bulk delete, and no "hide" primitive for media.

**Fix direction:** split read from destroy. Keep Moderator view (triage needs it), move destructive delete to
`is_admin()`, and add a _reversible_ moderation state. But see finding 1's architectural note: because media
is served from a public bucket, a `hidden` flag in the DB revokes nothing — a real "hide" needs the app or
the edge in the serving path, so the honest v1 is "Admin-only delete + variants + purge + audit", not a
cosmetic flag.

---

## 8. Serving, hotlinking, and bandwidth

- **How:** direct from the public R2 custom domain. The API returns an absolute `file_url`
  (`build_public_file_url`, `media/url.rs:3-11`), and nothing in `web/` proxies it — the service worker
  explicitly ignores the host (`web/src/service-worker.ts:620`). The API never streams image bytes.
- **Cacheability:** the app sets **no `Cache-Control`** on `put_object` (`controller.rs:417-431,471-480` — no
  `.cache_control()` anywhere in `rust/backend/api/src`; `grep -rn cache_control` only hits the quran
  response cache). Whether Cloudflare caches the object, and for how long, is **unverifiable from the repo**
  (no zone config, no Cache Rule, no Worker — `E5` §1/E5 F10 found the same absence for `/offline/*`).
  Either way the app cannot purge it (finding 6).
- **Hotlink/referer protection:** none. No `Referer` check, no signed URLs, no token, no per-IP limiter at
  the storage edge (R2 custom domains have no rate limiting by default; Cloudflare WAF/Rate Rules are
  zone-level and not in the repo).
- **Bandwidth exposure:** R2 has **no egress fees**, so third-party hotlinking costs the operator ~nothing in
  transfer; the exposure is (a) Class A/B operation counts at scale, (b) the brand-domain content risk
  (finding 2), and (c) _reputation_ — the bucket is a free CDN for anyone who copies a URL, including for
  content the operator deleted (finding 1). The one place this _does_ cost the VPS is the API's own list/view
  calls (pool of 1, A7), which are 20 rows/page and rate-limited at 30/min — not a material DoS.
- **No revocation primitive** is the structural point: because the bucket is public and the app is not in the
  path, every takedown story ends at "delete the object + purge the edge". The code does neither completely.

---

## 9. Fix set — ordered, with the takedown path first

**P0 — make takedown actually remove the content (ship first; the plan promises this).**

1. **Delete all variant objects in `media_v1::controller::delete`** (`rust/backend/api/src/modules/media_v1/controller.rs:803-860`).
   - Add `MediaVariant::list_keys_by_media_id(conn, media_id) -> Vec<String>` to
     `rust/backend/api/src/db/sea_models/media_variant/actions.rs` (mirror `media_usage::find_by_media_id`).
   - Read the variant keys **before** any delete; delete the original **and** every variant key (S3
     `delete_object` is idempotent — a missing key returns 204, so retries are safe).
   - If any S3 delete fails: return `FileDeletionError` **without** deleting any DB rows, and log the failed
     keys. Do not rely on the FK cascade for the variant rows; delete them explicitly only after every object
     delete succeeded.
   - Verify: an integration test that uploads a fixture that _does_ produce variants (a heavy PNG, so
     `should_skip_for_quality` does not fire), records the variant keys from the DB, deletes, and asserts
     every key 404s on the storage endpoint (or is absent from a `list_objects_v2` of the prefix). Today no
     such test exists (`grep -rn "media/v1/delete" rust/backend/api/tests` → 0).
2. **Tombstone before deletion** (new table `media_deletions`: `id, media_id, object_key, content_hash,
uploader_id, variant_keys_json, actor_id, reason, created_at`). Write it in the same transaction as the
   row delete; it is the audit row `audit_logs` never got, the retry list for a partial failure, and the
   `media_meta` evidence `E1` §3 step 6 wants captured before a takedown. Verify with a test asserting the
   row exists after a successful delete and that a failed object delete leaves both the media row and no
   tombstone.
3. **Orphan sweeper** (`just media-sweep`, manual, dry-run default): `list_objects_v2(prefix="media/")` →
   for each key whose base (`rsplit_once('.')`, then split on `@`) is not in `media.object_key` and not in
   `media_variants.object_key`, and whose `LastModified` is older than 24 h, delete. This is the only way to
   remove the orphans already created by this defect and by partial uploads. Verify: dry-run output on a
   seeded bucket; a unit test on the key-classification function (the risky part) covering
   `@480w`, `@lqip`, both extensions, and a live base key.
4. **Legal-hold guard:** once `E1`'s `legal_holds`/`case_evidence` tables exist, refuse media delete when the
   media is referenced by an open case (and force capture first). Until then, at minimum make the tombstone
   (step 2) the capture, and put the sequence in `docs/`.

**P1 — close the content-integrity hole.**

5. **Magic-byte validation** in `media_v1::controller::create` after `validate_upload` (`:236-240`):
   require `image::guess_format(&file_bytes)` ∈ the allowlisted raster set **and** `imagesize::blob_size`
   to succeed; reject otherwise with `InvalidFileType`. Decide the GIF question in the same change: either add
   the `gif` feature to the `image` crate (`Cargo.toml:137`) so GIFs decode, or remove `image/gif`/`.gif`
   from `validator.rs:8-17` so the allowlist matches the decoder set. Verify: table-driven tests with
   (a) HTML declared `image/png` → rejected, (b) a real JPEG declared `image/png` → accepted with the _real_
   format winning, (c) GIF → accepted per the decision.
6. **Re-encode the original by default** (metadata strip + polyglot strip): set
   `OPTIMIZER_KEEP_ORIGINAL=false` in `docker-compose.yml` and remove/relax the `significant_reduction`
   early-out for the original (`image_optimizer.rs:554-556`), plus `apply_orientation` before encoding
   (`:523-572`). Note honestly: magic bytes (step 5) narrow the arbitrary-bytes class but do not eliminate
   polyglots; only re-encoding does.
7. **Bound the decode cost**: cap width/height as well as total pixels in `image_optimizer::optimize`
   (`:171-182`), lower `OPTIMIZER_MAX_PIXELS` for the ≥2 GB host, and put the decode behind a small
   semaphore (the `PASSWORD_WORK` pattern, `services/password_work.rs:8-15`) so N concurrent uploads cannot
   each allocate ~50–150 MB. Verify with a test asserting the semaphore bounds concurrency (or a load test
   with 20 parallel heavy uploads).

**P2 — cost, audit, and the role split.**

8. **Per-uploader quota, enforced** (new `media/quota.rs` or a check in `create`): count + byte sum over
   `media` (and `media_variants`) for `uploader_id`, cap at e.g. 500 items / 100 MB, reject with
   `OperationNotAllowed`. Needs an index on `media.uploader_id` and `media.content_hash` (none exist —
   `m000001_init.rs` has zero `CREATE INDEX`). Add a per-account daily upload cap via the existing
   `abuse_limiter` (`media-upload:{user_id}`), since the nest limiter is per-IP.
9. **Audit the destructive verbs**: write an `audit_logs` row (or the tombstone) for media delete and for
   every admin role change; today neither is logged (E1 F1/C3 F7).
10. **Restrict delete to Admin** (`can_delete_media`, `controller.rs:543-548` → `actor.is_admin()`), keep
    Moderator read. If a Moderator needs an emergency verb, give them a _recorded_ one (tombstone + reason),
    not a silent one. Verify the existing tests at `:978-995` are updated deliberately, not deleted.
11. **Explicit cache policy + purge**: set `Cache-Control` on `put_object` (short `s-maxage`, e.g. 1 h, so a
    takedown self-heals even without purge) and add a Cloudflare cache-purge call on delete behind a scoped
    token (new env, e.g. `CF_CACHE_PURGE_TOKEN`), purging the original + variant URLs. If purge is judged not
    worth the new credential, the short TTL becomes the documented mitigation and the runbook must say
    "expect up to TTL of residual serving". Verify: a test that the purge call is attempted with the exact
    URL set, and a manual live check (unverifiable from the repo today).

**P3 — correctness hygiene (small, do them with P0/P1).**

12. **User erasure must remove media**: in `user::actions::admin_delete`
    (`db/sea_models/user/actions.rs:486-497`), collect the user's media (rows + variant keys) and delete the
    objects + rows in the same transaction (or write them to the tombstone and let the sweeper finish). Right
    now the FK is `ON DELETE SET NULL` (`m000001_init.rs:45-46`), so the content survives ownerless and
    public. Verify: a test that after `admin_delete`, no `media` row and no bucket object for that uploader
    remains.
13. **Fix the URL shape and pin it**: `build_public_file_url` (`media/url.rs:3-11`) emits
    `{public_url}/{bucket}/{key}` while the quran path emits `{public_url}/tanzil/...` with no bucket
    (`quran_v1/controller.rs:858,960,978`). One of the two is wrong against the live domain
    (**unverifiable from the repo** — verify with one `curl` of a real object and of a `tanzil/` artifact).
    Add a unit test for whichever shape is correct.
14. **Clear `avatar_id` on media delete** (it has no FK — `m000001_init.rs:10` — so it dangles), and write
    variant DB rows only after each variant PUT succeeds (`controller.rs:456-486`) with cleanup on failure.

---

## 10. Attacking my own fixes

- **Fix 1 (delete variants) — what breaks.** (a) The variant-key read and the S3 deletes are not atomic: if
  the process dies after deleting 3 of 6 variants, the rows survive (by design) and a retry re-deletes
  (idempotent) — good. (b) If the operator _wants_ to keep an object (legal hold), the new code will happily
  destroy it — the tombstone is not a hold. That is why step 4 exists, and until `legal_holds` ships, the
  runbook must say "capture first". (c) A deleted-then-recreated media (new upload) gets a fresh UUID, so no
  collision with the tombstone. (d) **The fix cannot help objects orphaned by past deletes** — only the
  sweeper can, which is why step 3 is not optional.
- **Fix 3 (sweeper) — the dangerous one.** A bug in the base-key classifier deletes live media. Mitigations
  built in: dry-run default, 24 h grace, cross-check both tables, and a unit test on the classifier. Residual
  risk: an object written but whose DB row insert is still in flight (upload in progress) — the grace window
  covers it; do not run the sweeper with a 0 h window. Also: the sweeper needs `ListBucket` on the media
  prefix — the app's existing credentials likely already have it (they are bucket-wide R2 keys), which is a
  reason to prefer a _separate_ key with prefix-scoped permissions for the sweeper.
- **Fix 5 (magic bytes).** Narrows, does not sanitize: a valid-image-prefix polyglot still passes. It also
  creates a new failure mode: a legitimate-but-exotic image (CMYK JPEG variants, APNG, HEIC renamed to .jpg)
  gets rejected. The allowlist and the decoder set must be reconciled in the same change (the GIF point), and
  the error message must say what to do. Also note the ordering: magic-byte check must come **after**
  `validate_upload` (so the extension/MIME are normalized) but **before** the dedup/moderation block, so
  unvalidated bytes never reach the provider.
- **Fix 6 (always re-encode).** Costs CPU on every upload (bounded by the 2 MiB cap and the new semaphore),
  changes the stored bytes (lossy for JPEG — a media library may want the original), and breaks the
  "original preserved" property that `media.content_hash` and dedup implicitly assume (the hash is of the
  _uploaded_ bytes, not the stored ones — after re-encoding, `find_by_hash` will no longer match a re-upload
  of the same source). If lossless preservation is required, the alternative is a metadata-strip pass that
  keeps pixels (JPEG: drop APPn/COM segments; PNG: drop ancillary chunks; WebP: drop EXIF/XMP) — a new
  dependency and its own tests. Decide explicitly; do not ship "re-encode everything" by default without
  recording the trade-off.
- **Fix 7 (decode bound).** A semaphore on decode interacts with the pool-of-1 DB (A7): the semaphore must
  not be held while awaiting DB calls, or an upload burst starves the single connection. Keep the permit
  scoped to the `spawn_blocking` closure only. Lowering `MAX_PIXELS` can reject legitimate images (a 24 MP
  phone photo is 24 M px) — which is why the fix should be "bound the _decoded byte_ budget and the
  concurrency", not "lower the pixel ceiling" alone.
- **Fix 8 (quota).** `SUM(size)` over `media` counts the _original_ only; variants are extra, so the quota
  under-counts unless it joins `media_variants` (or stores a `total_bytes` on `media`). A user can free quota
  by deleting (intended). The quota must be checked **before** the R2 puts, or an attacker races the check.
  And the check itself adds a read to the upload path — acceptable (uploads are not the hot path).
- **Fix 10 (Admin-only delete).** Reduces the number of people who can act at 3 a.m. If the plan ever adds a
  backup operator with only Moderator, they cannot take down content. The honest resolution is the tombstone
  - a Moderator-visible "request deletion" path, or an Admin-granted break-glass role — not silently keeping
    the current capability.
- **Fix 11 (purge).** Needs a new Cloudflare token (blast radius: can purge the whole zone) and per-URL
  purge calls (URL-count limits per call on lower plans). Purge does not reach browser caches or third-party
  copies. It is a mitigation, not a guarantee; the short-TTL part is the durable half.
- **Fix 12 (erasure).** Deleting a user's media in `admin_delete` runs S3 calls inside a DB transaction —
  **do not** hold the SQLite write transaction across network calls (A7's single connection). Correct shape:
  collect the keys in the transaction, commit, then delete objects, then tombstone the failures for the
  sweeper. That ordering means a crash between commit and delete leaves orphans → the sweeper is again the
  backstop.
- **What none of these fixes do:** make the app able to revoke access. As long as the bucket is public and
  the app is not in the serving path, every takedown depends on R2 + Cloudflare being called correctly. If
  the plan wants a _guaranteed_ capability, the architectural fix is to serve media through the app (or a
  Worker) with a deny-list — a much bigger change, and the one that makes "hide" possible. State that as a
  named decision, not a silent gap.

---

## Findings

Severity = damage if the media surface is exercised (uploads exist / a takedown is attempted). **NEW** = no
prior report states the mechanism (checked by grepping the 39 reports for the mechanism, not the topic).

| #   | Title                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Sev                                     | Evidence                                                                                                                                                                                                                                                                                  | Fix                                                                                                                                      |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Delete removes only the original; every derived variant survives publicly, and the DB loses the keys at the same instant** — `delete_object` on `media.object_key` only; variant rows are the only key record and die by FK CASCADE; variant keys are `{base}@{width}w.{ext}` / `{base}@lqip.{ext}`, derivable from the original URL; no sweeper, no `list_objects`, no lifecycle, no purge. Reach is upload-dependent: variants exist only when the optimizer ran (decode ok, ≤12 M px, bpp above the skip threshold — `image_optimizer.rs:171-191,385-391`), so typical large JPEGs produce none; heavy PNGs/graphics produce up to 6. **NEW (F6 stated the mechanism; this is the verified reach + the exact surviving set + the fix shape).** | **Critical** (for the takedown promise) | `media_v1/controller.rs:832-849`, `:851`; `db/sea_models/media/actions.rs:82-102`; `migration/src/m000001_init.rs:195-210`; `media_variant/model.rs:26-33`; `controller.rs:433-449`; `image_optimizer.rs:496-516`; `media/url.rs:3-11`; `grep -rn delete_object rust/backend/api/src` → 1 | §9.1 (enumerate variant keys, delete all, rows only after success) + §9.2 tombstone + §9.3 sweeper                                       |
| 2   | **No content validation: declared MIME/extension only, bytes never sniffed — any ≤2 MiB payload is stored and publicly served as `image/*`** (polyglots stored verbatim because `keep_original=true`; declared MIME also wins over `guess_format` in `probe()`). The optimizer's "disguised upload" re-check (`:379-388`) validates the _declared_ value and is a no-op on the keep-original path. **NEW**                                                                                                                                                                                                                                                                                                                                          | **High**                                | `validator.rs:8-17,46-88`; `controller.rs:236-240,307-321,325,364-376,379-388`; `image_optimizer.rs:315-318`; `settings.rs:175`                                                                                                                                                           | §9.5 magic-byte gate + allowlist/decoder reconciliation; §9.6 re-encode                                                                  |
| 3   | **Uploads are unmoderated: `IMAGE_MODERATION_ENABLED` default false → no moderator at all; `NoOpModerator` is dead code; and a misconfigured enable (`ENABLED=true`, empty URL) logs a warning and accepts unmoderated** (fail-open on config, fail-closed only on provider error). Dedup runs _before_ moderation, so re-uploads are never re-classified. **NEW mechanism** (E1 F5 confirmed the absence; the fail-open config path, the dead NoOp, the ordering, and the declared-MIME-to-provider detail are new)                                                                                                                                                                                                                                | **High**                                | `main.rs:495-522`; `controller.rs:249-266,272-305`; `image_moderation/mod.rs:34-49`; no `IMAGE_MODERATION_*` in `.env.example`/`docker-compose.yml`                                                                                                                                       | §9.3 of E1 + boot refusal when `ENABLED=true` and the provider is unusable; move dedup after moderation; add the provider to the runbook |
| 4   | **No quota, no cap, no accounting: `media_usage` is a ledger, never enforced; the only limit is 30/60 s per IP; one account/IP can add ~~86 GB/day (~~$80/mo) with no alert or ceiling.** No index on `content_hash`/`uploader_id`. **NEW**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | **Med**                                 | `router.rs:129-132`; `config/mod.rs:12`; `media_usage/actions.rs:12-37`; `media/actions.rs:216-235`; `m000001_init.rs` (0 `CREATE INDEX`)                                                                                                                                                 | §9.8 per-uploader quota + daily cap + indexes                                                                                            |
| 5   | **A Moderator (lowest staff role) can delete ANY media — including system media — with no audit, no undo, no soft-delete; and reads every media row/URL. Blast radius: total media wipe by the role `E1` proposes for a backup triager.** Round 8's claim verified; the tests pin it as intended. **NEW detail** (the role-blast-radius framing and the missing reversible alternative)                                                                                                                                                                                                                                                                                                                                                             | **Med-High**                            | `controller.rs:539-548,978-995`; `db/sea_models/user/model.rs:49-51`; `migration/src/m000001_init.rs:29-47`                                                                                                                                                                               | §9.10 Admin-only delete + tombstone/reason; keep Moderator read                                                                          |
| 6   | **No revocation at the edge: media is served from a public R2 domain, the app sets no `Cache-Control` on `put_object`, and there is no cache-purge capability anywhere — a takedown cannot guarantee the original stops being served (edge TTL unverifiable from the repo).** **NEW**                                                                                                                                                                                                                                                                                                                                                                                                                                                               | **Med-High**                            | `media/url.rs:3-11`; `docker-compose.yml:148-149`; `controller.rs:417-431` (no `.cache_control()`); `grep -rn "purge_cache\|api.cloudflare.com" rust/backend/api/src` → mail only; `web/src/service-worker.ts:620`                                                                        | §9.11 explicit short TTL + purge-on-delete + runbook wording                                                                             |
| 7   | **EXIF is preserved by default (GPS/device/timestamps) and the strip path is conditional even when enabled** — `OPTIMIZER_KEEP_ORIGINAL=true` default; alpha/icon/decode-fail/pixel-overflow/skip paths all keep the original verbatim; the re-encode also requires a ≥5% size win. EXIF orientation is never applied (image 0.25 does not auto-apply), so variants can be rotated relative to the original. **NEW** (F6 flagged the flag; the conditional paths, the bpp skip, the alpha/icon early-outs and the orientation defect are new)                                                                                                                                                                                                       | **Med**                                 | `settings.rs:170-179`; `image_optimizer.rs:236-243,385-391,523-572`; `image-0.25.10/src/images/dynimage.rs:1161`; `.env.example`/`docker-compose.yml` (unset)                                                                                                                             | §9.6 re-encode/metadata-strip + `apply_orientation` + document the decision                                                              |
| 8   | **User erasure does not remove the user's media: `admin_delete` deletes only the usage ledger and the user row; `media.uploader_id` is `ON DELETE SET NULL`, so rows and R2 objects survive ownerless and public (with EXIF).** **NEW**                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | **Med**                                 | `db/sea_models/user/actions.rs:486-497`; `m000001_init.rs:45-46`                                                                                                                                                                                                                          | §9.12 collect keys in txn → commit → delete objects → tombstone failures                                                                 |
| 9   | **No atomicity between storage and DB in either direction: objects are PUT before rows (orphan objects on any DB failure), variant rows are pushed before their PUT and PUT failures are swallowed (dangling rows), and the delete path destroys rows for objects it did not delete.** Nothing in the repo can list objects, so the drift is invisible. **NEW**                                                                                                                                                                                                                                                                                                                                                                                     | **Low-Med**                             | `controller.rs:415-431,456-487,489-528`; `media_variant/actions.rs:8-38`                                                                                                                                                                                                                  | §9.1/§9.3/§9.14                                                                                                                          |
| 10  | **The media URL shape contradicts the quran artifact URL shape for the same `public_url`: media emits `{public_url}/{bucket}/{key}`, quran emits `{public_url}/tanzil/...` with no bucket. One is wrong against the live R2 custom domain — verify with one `curl` before trusting any media URL.** **NEW**                                                                                                                                                                                                                                                                                                                                                                                                                                         | **Low-Med** (needs live verification)   | `media/url.rs:3-11` vs `quran_v1/controller.rs:858,960,978`; `docker-compose.yml:149`; `.env.example:20`; `deploy/README.md:88`                                                                                                                                                           | §9.13 one curl + a pinned unit test                                                                                                      |
| 11  | **Decode-cost exhaustion: 12 M px budget ≈ 48 MB RGBA per upload (plus resize/encode buffers) with no concurrency cap — `spawn_blocking` has no semaphore, so parallel uploads from one IP (30/min, and the IP is spoofable per E5 F1) can allocate GBs on the ≥2 GB host.** Header-only pre-checks are correct (no classic decode bomb). **NEW**                                                                                                                                                                                                                                                                                                                                                                                                   | **Med**                                 | `settings.rs:174`; `image_optimizer.rs:171-182,199-209`; `controller.rs:351-376`; `services/password_work.rs:8-15` (the missing pattern)                                                                                                                                                  | §9.7 dimension cap + decode semaphore + byte budget                                                                                      |
| 12  | **Hygiene: `image/gif` is allowlisted but the `image` crate has no `gif` feature → every GIF is stored verbatim (never optimized, never metadata-stripped); `users.avatar_id` dangles after a media delete (no FK); the media list has no `page` cap and `content_hash` is unindexed; and the pipeline has no upload/delete integration test at all.** **NEW**                                                                                                                                                                                                                                                                                                                                                                                      | **Low**                                 | `validator.rs:13,17`; `Cargo.toml:137`; `image_optimizer.rs:153-160,199-209`; `m000001_init.rs:10`; `media/actions.rs:216-219`; `grep -rn "media/v1/delete" rust/backend/api/tests` → 0                                                                                                   | §9.5 (GIF decision) + §9.14 + tests from §9.1                                                                                            |

### What is NOT a finding (checked, negative)

- **IDOR for regular users:** none. `view`, `list/query`, `usage/details`, and post attachment are all
  owner-scoped (`controller.rs:539-541,563-566`; `validator.rs:129-151`; `controller.rs:655-663,736-739`;
  `post_v1/controller.rs:46-63`). Unauthorized ids in `usage/details` are indistinguishable from nonexistent.
- **Path traversal / key control:** the object key is `media/{YYYY}/{MM}/{uuid4}.{ext}` with the extension
  allowlisted — no user input reaches the key (`controller.rs:881-890`).
- **Cross-user dedup leak:** a hash hit owned by another user falls through to a fresh record and never
  returns the existing row/URL (`controller.rs:249-266`).
- **Moderation bypass via SVG/polyglot-as-image in a browser:** the stored `Content-Type` is always
  allowlisted `image/*`, and the R2 domain is a different origin from the app, so the stored bytes are not a
  same-origin XSS primitive — the harm is the file-hosting/illegal-content class (finding 2), not script
  execution.
- **Classic decompression bomb:** header-only probe + pixel check before decode means the API never decodes
  an over-budget image (finding 11 is cost, not a bomb).

---

## Appendix — verification commands (all read-only)

```bash
# the defect
rg -n "delete_object" rust/backend/api/src                                   # exactly 1: media_v1/controller.rs:835
sed -n '803,860p' rust/backend/api/src/modules/media_v1/controller.rs        # delete: original only
sed -n '195,210p' rust/backend/api/migration/src/m000001_init.rs             # media_variants FK CASCADE
rg -n "list_objects" rust/backend/api/src                                    # 0 hits (no sweeper)

# validation / moderation
sed -n '236,321p' rust/backend/api/src/modules/media_v1/controller.rs
sed -n '46,88p'  rust/backend/api/src/modules/media_v1/validator.rs
sed -n '495,522p' rust/backend/api/src/main.rs                               # enabled default false; URL-missing fail-open
rg -n "NoOpModerator" rust/backend/api/src                                   # definition + tests only
rg -n "IMAGE_MODERATION|OPTIMIZER" .env.example docker-compose.yml           # 0 hits

# variants / optimizer
sed -n '433,487p' rust/backend/api/src/modules/media_v1/controller.rs
sed -n '488,521p' rust/backend/api/src/services/image_optimizer.rs           # post widths + lqip
sed -n '236,243p;385,391p' rust/backend/api/src/services/image_optimizer.rs  # keep_original + bpp skip
rg -n "fn build_public_file_url" -A 9 rust/backend/api/src/db/sea_models/media/url.rs

# serving / quota / erasure
rg -n "cache_control" rust/backend/api/src                                   # quran response cache only
rg -n "quota" rust/backend/api/src                                           # 1 unrelated mail comment
sed -n '486,497p' rust/backend/api/src/db/sea_models/user/actions.rs         # erasure skips media
rg -n "FormData|multipart" web/src                                           # 0 hits (no client)
sqlite3 "file:rust/data/easyquran.db?immutable=1" "select count(*) from media; select count(*) from media_variants;"
rg -in "media" docs/                                                         # 0 relevant hits
```
