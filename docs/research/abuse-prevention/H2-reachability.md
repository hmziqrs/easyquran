# H2 — Reachability re-verification: media (G1), billing (E3), sync/bookmarks (G2), notifications/push (G3)

Auditor: reachability subagent. Date: 2026-10-12. Method: read the router/mounts, guards, migrations,
deploy config (`docker-compose.yml`, `.env.example`, `settings.rs`, `main.rs`, `Dockerfile.api`,
`.github/workflows/images.yml`, `justfile`), the web app (`web/src`), the round-8/9 reports
(`G1-media.md`, `G2-sync-ugc.md`, `G3-device-fanout.md`, `E3-billing.md`, `F3-admin-surface.md`), plus a
**live probe of production `https://easyquran.fyi/api`** (empty bodies / GETs only; no accounts created,
no writes that persist — the one newsletter subscribe probe 500'd before reaching the DB).
Every probe result below is **NEW** evidence (no prior report probed production).

## 0. Live production probe results (NEW)

Probe method: `POST /api/csrf/v1/generate` → session cookie + CSRF token, then POST each path with
`cookie + csrf-token` (empty JSON body). A mounted route answers 400/401/403/422; an unmounted route
answers 404. GETs need no token. Rate limiter: 60/min on csrf generate — well under.

| Probe                                                    | Result                                   | Reading                                                                                   |
| -------------------------------------------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------------- |
| `POST /api/auth/v1/register`                             | **400**                                  | **auth router mounted → `WEB_AUTH_ENABLED=true` in production** (would be 404 if off)     |
| `POST /api/bookmark/v1/sync`                             | **401**                                  | mounted; `verified` guard rejects anonymous                                               |
| `POST /api/media/v1/list/query`                          | **401**                                  | mounted; `verified + ROLE_AUTHOR` guard                                                   |
| `POST /api/billing/v1/checkout`                          | **401**                                  | mounted; `authenticated` guard                                                            |
| `GET /api/billing/v1/plans`                              | **500** `{"type":"DB_004","status":500}` | mounted, public, **500s anonymously today**                                               |
| `GET /api/billing/v1/access/1`                           | **200**                                  | mounted; `post_access` table exists                                                       |
| `POST /api/notification/v1/list`                         | **401**                                  | mounted; `authenticated` guard                                                            |
| `POST /api/device/v1/register`                           | **401**                                  | mounted; `verified` guard                                                                 |
| `POST /api/admin/route/v1/list`                          | **401**                                  | mounted; admin guard (control: proves 401-vs-404 signal works)                            |
| `POST /api/admin/seed/v1/presets`, `/seed`               | **404**                                  | **seed routes NOT compiled into the production binary**                                   |
| `POST /api/mail/v1/webhook/cloudflare`                   | **403**                                  | mounted; webhook verification rejects                                                     |
| `POST /api/newsletter/v1/subscribe` (valid body + token) | **500** `DB_004`                         | mounted public; 500s (no `newsletter_subscribers` table)                                  |
| `GET /` `https://easyquran.fyi/bookmarks`                | 200                                      | bookmarks UI page live                                                                    |
| `GET /login`, `/register`, `/account`, `/settings`       | 200                                      | auth + account UI live                                                                    |
| `GET /api/api/docs`                                      | 303                                      | swagger UI mounted (openapi feature is default-on; note for the plan, outside this scope) |

## 1. Production config defaults for the relevant flags (code + deploy)

| Flag / setting                                   | Code default                                   | In `docker-compose.yml`           | In `.env.example` | Production effective                                                                                                             |
| ------------------------------------------------ | ---------------------------------------------- | --------------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `WEB_AUTH_ENABLED`                               | **false** (`settings.rs:530`)                  | not set                           | `true`            | **true — proven live (register=400)**; `DEPLOYMENT.md:157` + `deploy/README.md:505` instruct flipping it on after the mail smoke |
| `MAIL_PROVIDER`                                  | `smtp` fallback (`main.rs:38`)                 | `${MAIL_PROVIDER:-none}`          | `cloudflare`      | non-none **required at boot** when WEB_AUTH is on (`settings.rs:551-558`) → real mail in prod                                    |
| `MAIL_RATE_LIMIT_ENABLED`                        | **true** (`main.rs:39`)                        | not set                           | not set           | true                                                                                                                             |
| `IMAGE_MODERATION_ENABLED`                       | **false** (`main.rs:499`)                      | not set                           | not set           | **false → no moderator at all**                                                                                                  |
| `OPTIMIZE_ON_UPLOAD` / `OPTIMIZER_KEEP_ORIGINAL` | true / true (`settings.rs:173-175`)            | not set                           | not set           | true / true                                                                                                                      |
| `FCM_ENABLED`                                    | **false** (`main.rs:424`)                      | not set                           | not set           | **false → no push**                                                                                                              |
| `PUBLIC_FCM_VAPID_KEY` (web)                     | —                                              | `${PUBLIC_FCM_VAPID_KEY:-}` empty | empty             | empty unless operator set it → push UI disabled                                                                                  |
| Billing PSP vars (STRIPE/POLAR/… ×11)            | env-gated `try_init` (`main.rs:241-416`)       | **none**                          | **none**          | **no provider initialized**; boot warns "Billing endpoints will return errors"                                                   |
| `CLOUDFLARE_EMAIL_WEBHOOK_SECRET`                | read, `unwrap_or_default()` (`main.rs:81`)     | not set                           | **absent**        | empty → `verify_webhook` rejects everything (`cloudflare.rs:159-163`)                                                            |
| `TURNSTILE_SECRET_KEY`                           | required for prod password registration        | not set                           | present           | operator-set (registration open with captcha)                                                                                    |
| `seed-system` cargo feature                      | **off** (`Cargo.toml` default = `["openapi"]`) | —                                 | —                 | off in CI (`images.yml:190-192`) and `just image-api` (`justfile:120-121`)                                                       |
| `SEED_*` env vars                                | **do not exist** (`grep SEED_ src/` → 0)       | —                                 | —                 | n/a                                                                                                                              |

---

## 2. Media (G1)

1. **Mounted unconditionally?** YES. `router.rs:129-132` nests `/media/v1` outside every `if`/feature —
   the only layer is the 30/60 s per-IP limiter. Live probe 401 (guard), not 404.
2. **Minimum privilege?** `verified + role ≥ ROLE_AUTHOR` (`media_v1/mod.rs:19-21`;
   `auth_guard.rs:72-89` `role_min`; `ROLE_AUTHOR = 1`, `auth_guard.rs:19`). **No self-promotion**:
   register hardcodes `User` (`auth_v1/validator.rs:49`); only `ROLE_ADMIN+` can create/update users and
   cannot assign a role above their own (`user_v1/mod.rs:24-36`; `user_v1/controller.rs:246-258,328-356`).
   So Author is an admin-granted role, and there is **no admin UI in `web/`** — grants are API-only
   (`POST /user/v1/admin/update/{user_id}`) or a DB edit. The only in-repo creator of Admin accounts is
   the seeder (not in prod, §6).
3. **UI?** None. `grep media/v1 web/src` (excl. tests) → 0; `FormData|multipart` → 0. API-only.
4. **Tables?** `media`, `media_variants`, `media_usage` exist (`m000001_init`); 0 rows in the dev DB.
   Live 401 (not 500) → handler wiring intact. No prod evidence of any row; the module is the only writer.
5. **Config?** Moderation **off** (default false, and no `IMAGE_MODERATION_*` in compose/.env.example);
   optimizer on with keep-original; R2 creds are boot-required (`ObjectStorageConfig::from_env` `.expect`s
   them) → storage is live and public (`S3_PUBLIC_URL=https://r2.easyquran.fyi` default).
6. **Verdict: TRULY-LATENT today** — requires a role nobody demonstrably has, no UI, 0 rows — **but one
   admin action from REACHABLE-API-ONLY**. The route is live and an Author needs no UI; a determined user
   with a granted role can upload immediately. Caveat: absence of an Author in prod is not provable from
   the repo; treat the role grant as the trigger (§5).

## 3. Billing (E3)

1. **Mounted unconditionally?** YES. `router.rs:160-163`; module split into public / authenticated /
   admin routers (`billing_v1/mod.rs:14-71`). Live probes: `/plans` 500 anon, `/access/1` 200 anon,
   `/checkout` 401.
2. **Minimum privilege?** `/plans`, `/access/{post_id}`, `/webhook/{provider}` = anonymous;
   `/checkout`, `/checkout/post`, `/subscriptions`, `/payments` = `authenticated` (any logged-in user,
   not even verified); admin CRUD = `ROLE_ADMIN`.
3. **UI?** None. `grep billing/v1 web/src` → 0; no pricing/checkout route.
4. **Tables?** **8 of 9 missing** — migrations stop at `m000011`; no billing migration exists. `plans`,
   `subscriptions`, `payments`, `invoices`, `discount_codes`, `post_purchases` → 500; `post_access`
   exists and works. Live-confirmed: public `/billing/v1/plans` returns `DB_004`/500 anonymously.
   The paywall path (`paywall.rs:136-161`, no empty-input early return — re-verified) 500s
   `/post/v1/list/published` and `/post/v1/query` for authenticated non-staff callers
   (`post_v1/controller.rs:113-152`; `/query` needs Author).
5. **Config?** No PSP env in compose or `.env.example`; all 11 providers are env-gated and skipped when
   absent → even if tables existed, checkout would 502 "provider not initialized". No billing flag to
   flip; "going live" needs migrations **and** provider env.
6. **Verdict: BROKEN** — the routes are live and the public one 500s **in production today** (confirmed
   anonymously). The _abuse_ findings (card-testing, open redirect) are not live because the pipeline
   dies at the missing tables before any PSP call; the _breakage_ is the live issue.
   - Promote: **E3-F3** (public 500, live-confirmed), **E3-F1/F2** (paywall 500s authenticated ordinary
     users on the post feed / post query; code-verified, repro'd by E3).
   - Latent behind migrations+provider env: F4/F5 (provider verifiers), F6 (card-testing), F7 (open
     redirect), F8 (raw payload retention), F9/F10 (revocation/dedup), F11 (502 semantics).

## 4. Sync/bookmarks (G2)

1. **Mounted unconditionally?** YES. `router.rs:174-177` `/bookmark/v1`, live probe 401.
2. **Minimum privilege?** `verified` (`bookmark_v1/mod.rs:8-11`). An ordinary user reaches this the
   normal way: register (`/auth/v1/register` live), receive the verification mail (boot gate forces a real
   mail transport when web auth is on), verify, sync.
3. **UI?** **YES — full UI.** `web/src/lib/bookmarks/domain.ts:20` calls `/bookmark/v1/sync`;
   the engine starts on auth in `web/src/routes/+layout.svelte:138` (`bookmarks.onAuthChanged(...)`) and
   `store.svelte.ts:446` (`#engine.start()`); the reader (`VerseTools.svelte`) and `/bookmarks` page
   (live 200) use the store. No flag gates it beyond `PUBLIC_API_BASE_URL` (set in prod compose).
4. **Tables?** `bookmarks`, `bookmark_folders` exist (`m000007`, `m000008`). Live 401 (not 500).
5. **Config?** 100/min per-IP nest limiter; no feature flag. Spot-checked the report's claims in code:
   200 mutations/round (`bookmark_v1/validator.rs:63-65`), unbounded snapshot with the explicit
   "unbounded by design" comment (`controller.rs:103-120`), folder upsert `ON CONFLICT` with no count cap
   (`bookmark_folder/actions.rs:31-61`), client-supplied unclamped `updated_at` (`validator.rs:57`).
6. **Verdict: REACHABLE** — ordinary verified user, exercised by the shipped UI today. **Not latent.**

   UGC half of G2 (same report, different routes):
   - Comments `/post/comment/v1/*` = `verified` (`post_comment_v1/mod.rs:14`) → **REACHABLE-API-ONLY**
     (no web UI; blog frontends outside this repo are the consumers). F7, F8, F9 live.
   - Likes = `authenticated`, `track_view` = public (`post_v1/mod.rs:56-67`) → F10 live (API-only).
   - Post writes (`/post/v1/create|update|autosave|query|…`) = `verified + ROLE_AUTHOR`
     (`post_v1/mod.rs:52-54`) → **latent, same Author-grant trigger as media**. F11/F12 conditional.

## 5. Notifications / push (G3)

1. **Mounted unconditionally?** YES. `router.rs:165-177` `/device/v1` + `/notification/v1`; live probes 401.
2. **Minimum privilege?** device register/list/delete = `verified` (`device_v1/mod.rs:13`);
   notification list/unread_count/mark_read/mark_all_read = `authenticated`
   (`notification_v1/mod.rs:15-20`); notification create = `ROLE_ADMIN` (`:9-13`).
3. **UI?** Device register/delete IS called by the web app (`web/src/lib/firebase/messaging.ts:142` via
   the settings push toggle) — but only when `PUBLIC_FCM_VAPID_KEY` is configured; production compose
   defaults it empty and the UI then shows "notifications not configured". Notification inbox = **0 web
   callers** (`grep notification/v1 web/src` → 0). So: UI-gated for device, API-only for inbox.
4. **Tables?** `devices`, `notifications` exist (`m000005`). Live 401 (not 500).
5. **Config?** `FCM_ENABLED=false` default and absent from compose/.env.example → **push fan-out off**.
   `MAIL_RATE_LIMIT_ENABLED=true`. `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` absent from templates → the
   complaint webhook rejects everything (`cloudflare.rs:159-163`) → N9's unauthenticated
   third-party-suppression chain is dead by default. Newsletter subscribe 500s live (no table).
6. **Verdict: REACHABLE-API-ONLY** for the device + in-app notification endpoints (any verified /
   authenticated user; no admin needed) — **push-specific findings TRULY-LATENT by config** (FCM off,
   VAPID empty); admin-gated findings latent pending an Admin (see §6 caveat).
   - Promote (reachable now): **N1** (device register token transfer — verified user, route live),
     **N4 read-path half** (`mark_all_read` N-commit amplification — authenticated user),
     **N10** (`/device/v1/list` returns raw token — verified user), **N5** (plan-relevant: no inbox
     surface for the future enforcement notice), **N7** (public newsletter route; 500 confirmed live).
   - Latent-by-config: **N2/N3** (fan-out cost/prune — need `FCM_ENABLED=true` + VAPID),
     **N9** (needs the webhook secret set).
   - Latent pending Admin: **N6** (`admin/issue_code`), **N8** (`newsletter/send`), **N11**
     (`notification/create` no rank check), **N12** (minor guard gap).

---

## 6. Verdict table

| Subsystem               | Mounted            | Min privilege (prod)                                       | UI                                   | Tables             | Config         | **Verdict**                                                                                     |
| ----------------------- | ------------------ | ---------------------------------------------------------- | ------------------------------------ | ------------------ | -------------- | ----------------------------------------------------------------------------------------------- |
| Media (G1)              | yes, unconditional | verified + **Author** (admin-granted; no self-promotion)   | none                                 | exist, 0 rows      | moderation off | **TRULY-LATENT** (role nobody demonstrably has; no UI) — one role grant from REACHABLE-API-ONLY |
| Billing (E3)            | yes, unconditional | anon (public routes) / authenticated / Admin               | none                                 | **8 of 9 missing** | no PSP env     | **BROKEN** — live anonymous 500 confirmed; abuse latent behind missing tables                   |
| Sync/bookmarks (G2)     | yes, unconditional | **verified** (normal registration path)                    | **yes (live)**                       | exist              | no flag        | **REACHABLE**                                                                                   |
| Notifications/push (G3) | yes, unconditional | verified (device) / authenticated (inbox) / Admin (create) | device only, VAPID-gated; inbox none | exist              | FCM off        | **REACHABLE-API-ONLY** (device + inbox); push findings TRULY-LATENT by config                   |

Note on Admin existence: the repo has **no documented first-admin bootstrap** and no admin UI; the only
in-repo Admin creator is the seeder (not shipped). `deploy/README.md:411-419` documents admin-session
`/admin/bans` usage, which implies an Admin exists operationally. Every admin-gated finding should be
planned as "one admin action from live", not as unreachable.

## 7. Promote-to-plan list (reachable today → must be promoted)

| Finding                                                                                                                   | Why promote                                                                  |
| ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| **G2-F1** folder upsert no cap (1.2M rows/h, ~340 MB/h/account)                                                           | verified user, UI-driven sync, live                                          |
| **G2-F2** unbounded snapshot re-sent every round (~216 MB per response after 1 h; ~660 GB of responses in the first hour) | same; server-side comment admits "unbounded by design"                       |
| **G2-F3** per-IP-only limit; legit heavy user ~3.4 GB/day                                                                 | same                                                                         |
| **G2-F4** client-supplied unclamped `updated_at` locks an account's devices                                               | same                                                                         |
| **G2-F5** LWW, no tombstones, silent drops                                                                                | same                                                                         |
| **G2-F7/F8/F9** comment edit-after-flag; flag-clear revert; self-flag/queue flooding                                      | comments = verified → REACHABLE-API-ONLY                                     |
| **G2-F10** like no unique; view dedup in-memory/fail-open; 288k rows/day/IP                                               | likes authenticated + view public → REACHABLE-API-ONLY                       |
| **G2-F13** missing indices on the UGC/bookmark tables exercised above                                                     | same reachable set; pool-of-1 amplification                                  |
| **E3-F3** public `GET /billing/v1/plans` 500s anonymously                                                                 | **live-confirmed in production**                                             |
| **E3-F1/F2** paywall 500s the authenticated post feed / author query                                                      | authenticated ordinary user path, code-verified; no empty-input early return |
| **G3-N1** device register accepts arbitrary token; silent victim-token transfer                                           | verified user, route live; UI exists when VAPID set                          |
| **G3-N4 (read-path)** no per-recipient cap; `mark_all_read` N commits                                                     | authenticated user, route live                                               |
| **G3-N10** `/device/v1/list` returns raw FCM token                                                                        | verified user, route live                                                    |
| **G3-N5** no web caller for the notification inbox; push opens generic `/account`                                         | plan-relevant: enforcement-notice path has no surface                        |
| **G3-N7** newsletter confirm link targets a nonexistent web route; subscribe 500s                                         | public route, **live 500 confirmed**                                         |

Not promoted (with reason): all G1 findings (latent role); E3-F4…F12 (latent behind tables/providers;
F12 informational); G2-F6 (no fix needed — state the invariant); G2-F11/F12 (Author-gated); G3-N2/N3
(FCM-off), N6/N8/N9/N11 (admin/config-gated), N12 (minor).

## 8. Trigger list (TRULY-LATENT → what makes it live → what the plan must do)

| Subsystem / finding                                                 | Trigger                                                                                      | Plan action                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Media (all G1 findings 1-12)                                        | **one Admin role-grant to Author** (seconds, API-only) or the seeder being run; no UI needed | Do NOT treat as safe. Gate any media UI/Author-grant rollout on G1-1 (takedown deletes only the original — Critical for the takedown promise), G1-3 (moderation default-off/fail-open), G1-4 (no quota). Add the guards now; they are cheap and independent of shipping the UI. |
| Billing abuse findings (F4-F11)                                     | apply the 8 missing migrations + set one PSP env                                             | Plan note: migrations are the gate; when they land, F6 (card-testing) and F7 (open redirect) go live with the same commit. Keep F6/F7 fixes in the same PR as the migrations.                                                                                                   |
| Push findings N2/N3 (+ N9)                                          | `FCM_ENABLED=true` + VAPID + service-account; `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` set          | Plan note: one flag flip turns fan-out cost/prune findings live; the N9 webhook chain arms itself the moment the secret is configured. Fix before flipping.                                                                                                                     |
| Admin-gated findings (media role grant, N6, N8, N11, billing admin) | an Admin existing — operationally implied (`/admin/bans` docs) but no bootstrap in repo      | Plan note: since the repo ships no admin-creation path, either document a safe bootstrap or expect the seeder to be run against prod — which is exactly why the seeder guard (§9) matters.                                                                                      |
| G2-F11/F12, post writes                                             | Author grant (same trigger as media)                                                         | Fold into the media/Author trigger workstream.                                                                                                                                                                                                                                  |
| Seeder Admin accounts (F3-1)                                        | operator builds `seed_now`/TUI and points `DATABASE_URL` at a prod copy                      | Add `is_production()` refusal (4 call sites) + drop `password==email`/seeded Admin (§9).                                                                                                                                                                                        |

## 9. The single biggest ordering lever

**Media.** One admin role-grant (a single `POST /user/v1/admin/update/{user_id}` — seconds, no UI, no
migration, no flag) turns the whole unmoderated pipeline live: any ≤2 MiB payload stored and publicly
served as `image/*` on `r2.easyquran.fyi` (G1-2), no quota (G1-4), moderation off by default (G1-3), and
the broken takedown (G1-1) — and none of it needs a client, because the route is API-only anyway.
Runner-up: `FCM_ENABLED=true` (one flag) arms the fan-out findings; billing is _not_ a single flip (it
needs migrations + provider env). G2 needs **no trigger at all — it is already live**.

## 10. Seeder verdict (round 8 F3-1)

**Code claim verified.** `services/seed/base.rs:171-188`: 50 users, `password = user.email`,
`is_verified = true`, ~10% `UserRole::Admin` (custom.rs:135-155 uses a 1-in-3 Admin chance).
`grep is_production services/seed tui scripts/seed_now.rs scripts/ruxlog_tui.rs` → **0 hits**: no prod
guard anywhere. `seed_now.rs` reads `DATABASE_URL` (fallback `../../.env.dev`).

**Is the seeder binary in the shipped image? NO.**

- CI: `cargo build --release --locked -p ruxlog --features vendored-openssl` (`images.yml:190-192`);
  `just image-api` identical (`justfile:120-121`). `seed-system` is not in `default`
  (`Cargo.toml` default = `["openapi"]`).
- `Dockerfile.api` COPYs exactly one binary (`/app/ruxlog`) plus the provisioning script/manifest; the
  `seed_now`/`ruxlog_tui` bins are `required-features = ["seed-system"]`; `tui` and the `/admin/seed/v1`
  nest are `#[cfg(feature = "seed-system")]`.
- **Live-confirmed**: `POST /api/admin/seed/v1/presets` and `/seed` → **404** in production (a mounted
  route answers 401 — control probe `/admin/route/v1/list` = 401). `docker-compose.yml` invokes no
  seeder; the runtime image has no cargo/source.

**Can it run against production? Only with operator-level access.** The prod SQLite file lives in the
`api_data` Docker volume on the VPS; the compose stack publishes no ports (Traefik routes only `/api`),
and the runtime image contains neither the seed binary nor a compiler. Running it requires: shell access
to the VPS **and** a hand-built seed-system binary (or a prod DB copy + `DATABASE_URL` override).
No ordinary user, and no remote request, can reach it.

**Exact reachability: dev-only annoyance / operator footgun — NOT a Critical production issue today.**
The sharp edge is that the repo has **no other in-repo Admin creator** (§6) and no documented bootstrap,
so an operator who wants an Admin has an incentive to run the seeder — at which point guessable verified
Admin accounts exist in prod with no provenance. Dev DB spot-check: 13 users, **all `role=user`** — the
seeder is not reflected in the checked-in dev DB either.

**Plan action (keep, as hardening, not as a live incident):** `is_production()` refusal at the four entry
points (`seed_now::main`, `ruxlog_tui::main`, `seed_all_with_progress`, `seed_custom`); drop
`password == email` and seeded Admin roles (random password printed once, or no Admin); provenance tag on
seeded rows. Trigger for the fix: any decision to document/perform a production Admin bootstrap.

---

### Appendix — verification commands (read-only)

```bash
# mounts + gates
sed -n '72,190p' rust/backend/api/src/router.rs              # media 129-132, billing 160-163, device/notification/bookmark 165-177, seed 155-158
sed -n '17,22p;72,89p' rust/backend/api/src/middlewares/auth_guard.rs
sed -n '1,22p' rust/backend/api/src/modules/media_v1/mod.rs
sed -n '13,72p' rust/backend/api/src/modules/billing_v1/mod.rs
sed -n '8,12p' rust/backend/api/src/modules/bookmark_v1/mod.rs
sed -n '8,23p' rust/backend/api/src/modules/notification_v1/mod.rs
sed -n '24,36p' rust/backend/api/src/modules/user_v1/mod.rs    # admin-only role grants

# flags / deploy
sed -n '499p;424p;39p' rust/backend/api/src/main.rs
sed -n '530p' rust/backend/api/src/config/settings.rs
grep -n 'FCM\|IMAGE_MODERATION\|WEB_AUTH\|MAIL_PROVIDER\|SEED' docker-compose.yml .env.example
sed -n '187,192p' .github/workflows/images.yml
sed -n '119,125p' justfile
grep -n 'COPY' deploy/Dockerfile.api

# seeder
sed -n '171,188p' rust/backend/api/src/services/seed/base.rs
sed -n '1,25p' rust/backend/api/scripts/seed_now.rs
grep -rn 'is_production' rust/backend/api/src/services/seed/ rust/backend/api/src/tui/ rust/backend/api/scripts/seed_now.rs

# live probes (2026-10-12)
# POST /api/csrf/v1/generate -> cookie+token, then:
#   POST /api/{auth/v1/register,bookmark/v1/sync,media/v1/list/query,billing/v1/checkout,
#              notification/v1/list,device/v1/register,admin/route/v1/list,
#              admin/seed/v1/presets,admin/seed/v1/seed}
# GET  /api/billing/v1/plans  -> 500 DB_004 (anonymous)
```
