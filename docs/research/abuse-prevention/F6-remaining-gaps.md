# F6 — Final gap enumeration (round 6, closing pass)

Enumerator: final gap enumerator. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` (working tree, read-only).
Method: `ls` + `head -40` over all 39 `*.md` reports to build the covered map; keyword grep over all reports to
find topic holes; then **read the actual code** for the top holes. Every `file:line` below was read this session.
`NEW` = no prior report states the mechanism (checked by grepping all 39 files for the mechanism, not the topic).

---

## 1. Covered / uncovered map

### 1.1 The 39 reports (one line each)

| #    | Report                                                                                                                                  | Topic                                                                                                                                     |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| R2×7 | ban-operations / commercial-products / evasion-counters / fingerprint-science / legal-privacy / network-transport / session-binding-rba | External research: ban models + appeals + evasion + fingerprint entropy + law + transport-layer signals + DPoP/RBA standards              |
| A1   | threat-model                                                                                                                            | Adversarial kill chain against DESIGN-v0 device identity                                                                                  |
| A2   | false-positives                                                                                                                         | FP/collateral damage; cohort key; shared hardware                                                                                         |
| A3   | repo-integration                                                                                                                        | Design-vs-repo fit; session wire format; `BanUnit`; migration atomicity                                                                   |
| A4   | web-collisions                                                                                                                          | Web/SSR correctness vs the delivery contract; device token shapes                                                                         |
| A5   | privacy-legal                                                                                                                           | GDPR/ePrivacy/DPDP compliance of the device layer                                                                                         |
| A6   | testability                                                                                                                             | CI gates; happy-dom limits; can the design be verified                                                                                    |
| A7   | performance-cost                                                                                                                        | Pool-of-1, fsync, per-request cost envelope                                                                                               |
| A8   | ops-realism                                                                                                                             | One-operator T&S realism; admin surface inventory; evidence model                                                                         |
| A9   | crypto-keys                                                                                                                             | `COOKIE_KEY`/`FIELD_ENC_KEY`/CSRF/HMAC/rotation/erasure; Turnstile fail-closed                                                            |
| A10  | data-model                                                                                                                              | Schema, migration risk, ban placement, idempotency                                                                                        |
| B1   | token-storage                                                                                                                           | Cookie vs localStorage/IDB for the device token (Q1)                                                                                      |
| B2   | contrarian                                                                                                                              | Is fingerprinting the right tool; costed alternatives A/B/C                                                                               |
| B3   | anonymous-abuse                                                                                                                         | Reader path: no limiter, one cookie defeats the cache, single global SSR bucket                                                           |
| B4   | rules                                                                                                                                   | v1 detection rule set, thresholds, evasion analysis                                                                                       |
| B5   | shared-devices                                                                                                                          | Family/madrasa/café shared-device positive design                                                                                         |
| B6   | rollout                                                                                                                                 | Flags, shadow mode, rollback, live-DB migration safety                                                                                    |
| B7   | drift                                                                                                                                   | What rotted; which invariants are unguarded; CI gates deleted at HEAD                                                                     |
| B8   | retention-exec                                                                                                                          | Can it delete, prove it, back up, destroy the pepper — no                                                                                 |
| B9   | second-order                                                                                                                            | What v1's fixes break                                                                                                                     |
| B10  | falsification                                                                                                                           | Re-check of round-3/4 claims; contradictions                                                                                              |
| C1   | optionAB-audit                                                                                                                          | Adversarial audit of the A+B plan (login limiter, Gmail dot-alias)                                                                        |
| C2   | contradictions                                                                                                                          | Referee of B10 vs tree/deps                                                                                                               |
| C3   | base-rate                                                                                                                               | Measured abuse base rate: structurally absent                                                                                             |
| C4   | shipped-debt                                                                                                                            | Live privacy/compliance debt (policy falsehoods, consent, deletion)                                                                       |
| C5   | decision-ledger                                                                                                                         | Decisions D1.. with evidence; WS0–WS9 shape                                                                                               |
| C6   | plan-shape                                                                                                                              | Plan document shape, target paths, docs restructure                                                                                       |
| D1   | verification                                                                                                                            | 12 TRUE / 2 PARTLY TRUE on round-5 claims                                                                                                 |
| D2   | v1-attack                                                                                                                               | Attack on WS0–WS9; OAuth bypasses admission; WS2/3 mis-sized                                                                              |
| D3   | measurement-attack                                                                                                                      | C3's 7-day measurement plan not runnable/trustworthy                                                                                      |
| D4   | hard-rules                                                                                                                              | Plan vs AGENTS.MD; W1–W8 doc-truth; toolchain compliance                                                                                  |
| D5   | final-gaps                                                                                                                              | Vector table: mail oracle/caps, billing 500s, blog search, seed, R2 bucket, offline pack, OAuth caps, IR, TUI/admin                       |
| E1   | incident-response                                                                                                                       | Minimum viable IR; moderator is absent when disabled; no runbook                                                                          |
| E2   | mail-abuse                                                                                                                              | Mail end-to-end: 3-outcome oracle, provider caps, suppression fail-open                                                                   |
| E3   | billing                                                                                                                                 | Mounted module with no tables; paywall read path 500s                                                                                     |
| E4   | ban-contract                                                                                                                            | WS2a ban bundle; partial UNIQUE index; banned-UX classifier                                                                               |
| E5   | fresh-eyes                                                                                                                              | Independent pass: IP trust model, CSP `unsafe-eval` (API), password-work saturation, forgot-password oracle, mail, challenge-map eviction |
| E6   | reader-path                                                                                                                             | B3 claim verification + phase-0 reader fix set                                                                                            |

### 1.2 Uncovered after this pass (mechanism-level, grep-verified)

| Area                                                                             | Status before                                                                                                                                                                  | This report              |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------ |
| **Media upload pipeline mechanics** — variant objects, EXIF, quota, decode bombs | Only the moderator seam (`B2` F13, `C1` A9, `E1` §0) and the general "takedown needs the object removed" line (`E5` F7). `grep image_optimizer` over all reports → **0 hits**. | **AUDITED (§2.1) — NEW** |
| **Bookmark sync growth**                                                         | `B2` F10 only ("`expectedUserId` guards exist; outbox ≈ 0 abuse"). No report reads the sync endpoint.                                                                          | **AUDITED (§2.2) — NEW** |
| **`app_constants` secret persistence**                                           | `A8`/`C3`/`D3`/`D5` mention the table as a marker store; nobody reads `looks_like_secret_key`.                                                                                 | **AUDITED (§2.3) — NEW** |
| i18n/paraglide as an injection surface                                           | `A4` mentions paraglide; `D4` §2b covers the key/namespace gate. No injection read.                                                                                            | Triaged LOW (§3)         |
| Service worker cache poisoning / stale privileged responses                      | `A4` §4 + `B1` §4 cover the `/api/` bypass and the token angle only.                                                                                                           | Triaged LOW (§3)         |
| CSP nonce mechanism (per-request? unguessable?)                                  | `E5` F9 covers the **API** default CSP (`unsafe-eval`), not the web nonce.                                                                                                     | Triaged LOW — sound (§3) |
| Turnstile end-to-end (replay/hostname/action binding)                            | `A9` (fail-closed), `E2` (mail paths).                                                                                                                                         | Triaged LOW (§3)         |
| Dependency patch contents / deny.toml execution                                  | `B7` (CI gates removed), `D4` §2 (toolchain compliance).                                                                                                                       | Triaged LOW (§3)         |
| `.env` handling, `db/` runtime reads, DB-file permissions                        | `B8` (backup/perms context), `B3`/`E6` (provisioning).                                                                                                                         | Triaged LOW (§3)         |

---

## 2. The three audited holes

### 2.1 Media: a deleted image's variants survive — `NEW`

**Mechanism.** `media_v1::delete` deletes exactly one S3 object — the original — then deletes the DB row:

- `rust/backend/api/src/modules/media_v1/controller.rs:832-849` — `delete_object()` on `media.object_key` only.
- `rust/backend/api/src/db/sea_models/media/actions.rs:82-102` — `delete_by_id` deletes the `media` row.
- `migration/src/m000001_init.rs:195-210` — `media_variants` FK `ON DELETE CASCADE`, so **every variant DB row
  is destroyed at the same instant**. The DB rows are the only record of variant object keys.
- `grep -rn "delete_object" rust/backend/api/src/` → **exactly one call site** (the one above). There is no
  sweeper, no orphan reaper, no lifecycle rule in-repo.

**Why the leftovers are reachable.** Variant keys are mechanically derivable from the original URL:

- `controller.rs:409-413` — `base_object_key` = original key minus extension.
- `controller.rs:440-449` — variants are `"{base}@{width}w.{ext}"` and `"{base}@lqip.{ext}"`.
- `controller.rs:505-509` + `db/sea_models/media/url.rs:3-13` — the response hands out a **public bucket URL**
  (`build_public_file_url`), no auth on the object.

So after a takedown, `media/2026/10/<uuid>.jpg` 404s while `media/2026/10/<uuid>@480w.jpg`,
`@768w.jpg`, `@1024w.jpg`, `@1600w.jpg`, `@2048w.jpg`, `@lqip.jpg` still serve to anyone who has (or
cached, or re-derived) the original URL. For CSAM/illegal-image takedown this is the difference between
"removed" and "still up". This is the specific mechanism behind `E5` F7's general sentence — `E5` asks for the
object to be removed; the object _is_ removed and the variants still are not.

**Secondary, same code path.** Variant upload failures are logged and ignored (`controller.rs:471-486`), then
DB rows are written unconditionally (`:511-528`) → DB rows pointing at objects that may not exist (and, in the
delete direction, objects with no rows).

**EXIF (same surface, lower severity).** Default config keeps originals verbatim:
`src/config/settings.rs:173-175` — `OPTIMIZE_ON_UPLOAD=true`, `OPTIMIZER_KEEP_ORIGINAL=true`,
`OPTIMIZER_MAX_PIXELS=12_000_000`. The only metadata-stripping path is `reencode_original`
(`image_optimizer.rs:236-243`, gated on `!keep_original`), and images that fail decode (`:199-209`) or exceed
the pixel budget (`:171-182`) also keep the original bytes. Uploads are `verified` + `ROLE_AUTHOR`
(`media_v1/mod.rs:19-21`, admin-assigned per `E1` §0), so GPS/EXIF exposure is trusted-author only — but the
privacy policy (`A5` §0) says nothing about image metadata, and the object is world-readable once posted.

**Fix (smallest):** delete variant objects before the row delete (the keys are in hand at that moment), or
write them to a tombstone table and sweep; add a `MEDIA_VARIANT_*` retention/GC job; consider
`OPTIMIZER_KEEP_ORIGINAL=false` for non-alpha strategies or an EXIF strip on the keep-original path.

### 2.2 Bookmark sync: unbounded per-user folder growth + full-set snapshot per round — `NEW`

**Mechanism.** `POST /bookmark/v1/sync` is a write endpoint that also returns the caller's **entire** folder +
bookmark set every round:

- `rust/backend/api/src/modules/bookmark_v1/validator.rs:60-65` — ≤200 mutations per round; `:48-49` name ≤100.
- `rust/backend/api/src/modules/bookmark_v1/controller.rs:103-120` — "Snapshot is unbounded by design" —
  `list_for_user` for folders **and** bookmarks, both without LIMIT.
- `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs:104-111` — unbounded SELECT.
- `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs:31-56` — `INSERT … ON CONFLICT` with **no
  per-user cap**; folder ids are client-minted UUIDs (`validator.rs:46-47`), so there is no uniqueness
  pressure either.
- `rust/backend/api/src/router.rs:174-177` — the only limit is 100/min/IP.

Bookmarks are accidentally bounded: `bookmark/actions.rs:76-120` collapses rivals per `(surah, ayah)`, so
≤32,604/account (114 × 286 — the validator does not check per-surah ayah counts, G2 §1.1). **Folders are not.** 200 folders/request × 100 requests/min = 20k/min = **1.2M folders/hour**
from one verified account. Every subsequent sync returns all of them: after an hour that is a ~100 MB+ JSON
response built in memory, while the request holds the single pooled connection across the whole round
(`controller.rs:149-157`, pool = 1). On the documented ≥2 GB VPS this is a self-inflicted OOM/availability
kill, plus ~260–460 MB/hour of durable storage growth (G2 §1.2) — all from an account the plan already admits
(`verified` is the only gate; `expectedUserId` is not a cap).

**Secondary.** `updated_at` is fully client-supplied (`validator.rs:57`) with LWW semantics, so a client can
future-date a folder or verse (e.g. year 9999) and lock it against the account's other devices until then —
self-inflicted, but a support/data-integrity wart the sync workstreams should bound.

**Why prior reports missed it.** `B2` F10 checked this endpoint only for the `expectedUserId` cross-account
guard and concluded "abuse reduction ≈ 0". The growth axis was never measured.

**Fix:** per-user folder cap (and a bookmark cap as defence-in-depth), a byte/item budget on the snapshot with
cursor pagination (the code comment already names the seam), and a clamp on `updated_at` skew.

### 2.3 `app_constants`: the secret filter has a hole and `R2_ACCESS_KEY_ID` is stored plaintext — `NEW`

**Mechanism.** `AclService::bootstrap_from_env` imports every env var except those matching a denylist, with a
comment stating the invariant it does not hold:

- `rust/backend/api/src/services/acl_service.rs:46-47` — _"Security: never persist secret env vars into
  app_constants at rest."_
- `rust/backend/api/src/services/acl_service.rs:85-135` — the filter is a needle list + suffixes
  (`_KEY`, `_SECRET`, `_TOKEN`, `_PASSWORD`, …). `R2_ACCESS_KEY_ID` ends in `_ID`; the needle list contains
  `AWS_ACCESS_KEY_ID` but **not** `R2_ACCESS_KEY_ID` → not skipped.
- Dev DB proof (read-only): `SELECT key,is_sensitive FROM app_constants WHERE is_sensitive=1` returns
  **`R2_ACCESS_KEY_ID`** (32-hex value) among 195 rows. It is masked by the API
  (`admin_acl_v1/controller.rs:72-83`, `:98-108` — masking is correct) but sits in plaintext in the SQLite
  file/volume. Two local-tooling vars (`BROWSER_USE_ACCESS_VERIFYING_IDENTITY`, `CODEX_PERMISSION_PROFILE`)
  were imported too — dev hygiene, not prod.

**Severity: low-med.** An R2 access key **ID** alone does not authenticate (the secret key is filtered —
`R2_SECRET_ACCESS_KEY` contains `SECRET`), so this is an invariant violation and a hygiene leak, not a live
credential. It still matters because (a) the code asserts the opposite, (b) the table is where `C3` §4
proposes to record review markers, and (c) the filter is a denylist: the next secret whose name misses every
needle lands in the DB with no test to catch it.

**Fix:** add `ACCESS_KEY_ID`/`_ID`-style needles **or** invert to an allowlist of importable keys; add a unit
test asserting the known secret env names (`R2_ACCESS_KEY_ID` first) are refused.

---

## 3. Remaining uncovered areas — can they change the plan?

| Area                                                             | Assessment        | Why                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------- | ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| i18n / paraglide message injection, `i18n:check`, locale routing | **LOW**           | Catalogs are repo JSON (`web/messages/{en,ar}.json`, `messages/reader\|auth`); `check-i18n.mjs` is a key-coverage/namespace check that runs on `prebuild`/`precheck`/`prelint`/`pretest` (`web/package.json:12-33`). Locale selection is gated by `isUiLocale` before any catalog use (`hooks.server.ts:491-494`). No user-controlled string enters a message template; Svelte escapes.                                                                                                                                      |
| Service worker cache poisoning / stale privileged responses      | **LOW**           | `/api/` is bypassed (`service-worker.ts:613-623`); `isCacheable` rejects `private`/`no-store` (`:123-128`), which `applyHeaders` sets whenever a cookie is present (`hooks.server.ts:130-137`); logout purges user caches (`purgeUserCachesHandler`); disk-cache decisions require no cookie. The one gap worth a test: a same-origin GET whose response misses the private marker would be cached — but every hook-served response gets the marker, and non-hook (static/prerendered) responses are public by construction. |
| CSP nonce (per-request? unguessable?)                            | **LOW — sound**   | `freshNonce()` = `randomBytes(16).toString("base64")` per request (`hooks.server.ts:41-43`); disk cache stores a placeholder and substitutes per request (`:364`, `:386`); prerendered pages use boot-scanned sha256 hashes (`server.ts:167-203`); 304s deliberately omit CSP (`server.ts:261-270`). Residual: the hash map is keyed on exact pathname, so a trailing-slash variant falls back to hash-less CSP — fail-closed (JS-dead), not a security hole.                                                                |
| Turnstile end-to-end                                             | **LOW**           | Fails closed on unset-secret-in-prod and transport error (`turnstile.rs:42-47,74-82`); only `success` is parsed (`:83-88`) — `hostname`/`action`/`cdata` unvalidated, which is defence-in-depth (Cloudflare enforces single-use + hostname at the widget). Residual worth one line in the plan: bind `action` and record `hostname` in the abuse ledger.                                                                                                                                                                     |
| OAuth state/nonce/PKCE correctness                               | **LOW — covered** | PKCE S256 + state TTL 600 s + single-use native nonces verified (`services/oauth/state.rs:131-205`, `apple_auth_v1/controller.rs:39-49`); provider-farming caps covered by `D5` #8; the evict-oldest state map is `E5` F8.                                                                                                                                                                                                                                                                                                   |
| Dependency patch contents / `deny.toml` execution                | **LOW**           | Two patches, 58 lines total (`patches/*.patch`, hotkeys + svelte-store); `deny.toml` exists with a real advisory/licence policy but nothing invokes it in CI or `Justfile` (`B7` covers the removed gates; `D4` §2 covers the toolchain rules).                                                                                                                                                                                                                                                                              |
| `[slug].txt` / `llms-full.txt` routes                            | **LOW**           | Generated/static markdown-negotiation surfaces; already listed as deliberate (`A2`, `B3`); no user input.                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `.env` handling                                                  | **LOW**           | `.env`/`.env.*` ignored with `!.env.example` (`.gitignore:14-17`); tracked examples contain placeholders only (`CHANGE_ME_*`, empty secrets); `rust/.env.example` has `devkey`/`devsecret` dev defaults.                                                                                                                                                                                                                                                                                                                     |
| `db/` runtime reads / DB-file permissions                        | **LOW**           | App reads `db/quran/**` at runtime via env-configured paths with a baked dev fallback (`settings.rs:282-298`); container runs as non-root `appuser` with `/app/data` chowned (`Dockerfile.api:25,56`). Backup/retention coverage is `B8`; provisioning is `B3`/`E6`.                                                                                                                                                                                                                                                         |

---

## 4. Verdict — is the audit loop exhausted?

**Nearly. One narrow round is still worth running; a general round of 10 agents is not.**

What the 39 reports did cover is deep: the device/ban/crypto/legal/ops axis has been attacked from seven
angles, with a contradiction referee and a falsification pass on top. The last three rounds (D, E) mostly
produced confirmations, corrections of stale claims, and design decisions — that is the signature of a
converged loop.

But this pass falsifies the exhaustion claim in one specific place: **the user-generated-content and sync
surfaces were under-covered relative to how much the plan touches them.** Two of the three holes here are
plan-changing:

1. **Media variants survive deletion.** The plan's WS5 is the CSAM/image-moderation workstream; a takedown
   that leaves six derived copies public is a direct defect in the deliverable that workstream promises.
   `E5` asked for "object removed, not just a row"; nobody checked that the removal is complete.
2. **Bookmark sync has no per-user cap and an unbounded snapshot.** Any workstream that touches offline/sync
   (or the plan's cost/availability theme) inherits a live authenticated amplification vector.
3. `app_constants` is a smaller item (invariant + hygiene), but it is a one-line fix and it sits under a
   table the plan itself proposes to use.

So: **do not close yet — run one targeted round (3–4 agents, not 10) over the UGC/sync/storage surface:**
media pipeline end-to-end (variants, EXIF, moderation ordering, quota), bookmark/device sync growth and
conflict bounds, comment/post lifecycle including admin delete paths, and notification fan-out caps. After
that, the loop is exhausted: everything else I triaged lands in the LOW bucket, and the remaining findings
would be nits, not plan changes.

Honest caveat: I audited three holes in depth and triaged ~12 other candidates from source greps, not full
reads. The LOW ratings above are "no mechanism found from the entry points I read", not proofs of absence.
