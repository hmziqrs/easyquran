# E3 — Billing: the mounted module with no tables

Auditor: billing pass (follow-up to D5 items 2a/2b/2c). Date: 2026-10-12.
Method: read-only. Every repo claim is a `file:line`. The dev database was read with
`sqlite3 "file:rust/data/easyquran.db?immutable=1"` (no writes). Provider-signature claims were
checked against the providers' published specs (Paddle, Revolut; cited inline).

**Answer up front.** The billing module is mounted, rate-limited, CSRF-exempted at its webhook, and
wired to 11 env-gated PSP registrations — and **eight of the nine tables it needs do not exist in any
migration or runtime DDL**. The one table that does exist (`post_access`, `m000001_init.rs:175`) is
what makes the failure so specific: the paywall's _policy_ read succeeds and the _entitlement_ reads
fail, so `POST /post/v1/list/published` returns **500 for every authenticated non-staff user**, even
when the list is empty. Nobody has audited billing; this is the audit.

The correct v1 action is **not** to add the migrations. Adding them would turn a dead module into a
live payment surface with two providers whose webhook verifiers reject all genuine events, no
card-testing control, a provider-hosted open redirect, and no unique constraints. The smallest safe
change is: **degrade the paywall list reads (fail-closed on content) and unmount `billing_v1`** until
the pre-live checklist in §8 is done.

---

## 1. Existence audit

### 1.1 Table inventory

Migrations stop at `m000011` (`migration/src/lib.rs:3-14`). No migration mentions any billing table
(`rg 'post_purchases|subscriptions|payments|invoices|discount_codes|plans|payout' migration/src/` →
0 hits). Runtime DDL is four tables only: `sessions`, `rate_limit_state`, `translation_popularity`,
`user_oauth_identities`. The dev DB confirms (`sqlite3 … ".tables"`): no `plans`, `subscriptions`,
`payments`, `invoices`, `discount_codes`, `post_purchases`, `payout_accounts`, `payout_ledgers`.

| Table             | Model                                | Migration                       | Runtime DDL | Queried by (code)                                                                                                                                                      | Effect today                                   |
| ----------------- | ------------------------------------ | ------------------------------- | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `post_access`     | `db/sea_models/post_access/model.rs` | **yes** — `m000001_init.rs:175` | —           | `services/paywall.rs:72,90` (all viewers); `billing_v1/controller.rs:1165,1192`                                                                                        | Works (0 rows; only writer is the admin route) |
| `plans`           | `plan/model.rs`                      | **none**                        | none        | `public_list_plans` (`controller.rs:363-374`), `create_checkout` (`:387-395`), admin CRUD (`:90-227`)                                                                  | 500                                            |
| `subscriptions`   | `subscription/model.rs`              | **none**                        | none        | `paywall.rs:136-147`; `my_subscriptions` (`:508-524`); admin list/cancel (`:229-276`); `admin_delete_plan` count (`:210-214`); webhook arms (`:733-867`, `:1134-1156`) | 500; **paywall 500**                           |
| `payments`        | `payment/model.rs`                   | **none**                        | none        | `my_payments` (`:526-542`); admin list (`:278-288`); webhook `invoice.payment_succeeded` (`:886-919`), `payment.confirmed` (`:1020-1056`); refund (`:1111-1132`)       | 500                                            |
| `invoices`        | `invoice/model.rs`                   | **none**                        | none        | `admin_list_invoices` only (`:290-300`)                                                                                                                                | 500                                            |
| `discount_codes`  | `discount_code/model.rs`             | **none**                        | none        | admin list/create/delete only (`:302-361`)                                                                                                                             | 500                                            |
| `post_purchases`  | `post_purchase/model.rs`             | **none**                        | none        | `paywall.rs:101-113,115-134`; webhook grant (`:682-726`); refund delete (`:1093-1100`)                                                                                 | 500; **paywall 500**                           |
| `payout_accounts` | `payout_account/model.rs`            | **none**                        | none        | **nothing** — dead model (zero `Entity`/`ActiveModel` refs outside the model dir)                                                                                      | none                                           |
| `payout_ledgers`  | `payout_ledger/model.rs`             | **none**                        | none        | **nothing** — dead model (table name in the model is `payout_ledgers`)                                                                                                 | none                                           |

Related, out of scope: `newsletter_subscribers` (C3 F8) and `seed_runs` are also unmigrated.

### 1.2 The exact queries that 500, and the exact route

`POST /post/v1/list/published` (`post_v1/mod.rs:65`, public/auth-optional) →
`find_published_posts` (`post_v1/controller.rs:314`) → `apply_paywall_list` (`:113-152`; call at `:330`).

For any viewer that is authenticated and not a moderator (`viewer.filter(|_| !is_staff)`,
`controller.rs:122`), the tuple at `:122-126` evaluates:

1. `paywall::user_purchased_post_ids` (`paywall.rs:115-134`):
   ```sql
   SELECT "post_purchases"."id", … FROM "post_purchases"
   WHERE "post_purchases"."user_id" = ? AND "post_purchases"."post_id" IN (?, …)
   ```
   → `no such table: post_purchases` (reproduced against the dev DB). Sea-orm maps it to
   `DbErr::Exec`, `classify_db_error` finds no pattern (`error/database.rs:4-46`), → `ErrorCode::QueryError`,
   → HTTP **500** (`ruxlog-types/src/error.rs:247`).
2. `paywall::user_has_active_subscription` (`paywall.rs:136-161`):
   ```sql
   SELECT "subscriptions"."id", … FROM "subscriptions"
   WHERE "subscriptions"."user_id" = ?
     AND ("subscriptions"."status" = ? OR "subscriptions"."status" = ?)
   ```
   → `no such table: subscriptions` → same 500.

Asymmetry that matters: `user_purchased_post_ids` early-returns on an empty `post_ids` (`paywall.rs:121-123`);
`user_has_active_subscription` has **no early return**. So an authenticated non-staff user gets a 500
even when the published list is empty.

Second call site: `POST /post/v1/query` (`post_v1/controller.rs:399`, ROLE_AUTHOR) → `apply_paywall_list`
with `Some(user)` at `:432`. Author (non-moderator) → 500. Moderator/Admin → staff bypass → 200.

Third call site: `POST /post/v1/view/{id_or_slug}` → `apply_paywall_single` (`:95-110`) →
`paywall::user_has_access` (`:102`). `user_has_access` returns early when the policy is open
(`paywall.rs:171-176`), so **free posts are safe**; a post with a `Paid`/`SubscriberOnly` policy would
500 for authenticated non-bypass viewers. No such row exists today (dev DB `post_access` count = 0).

`feed_v1` (`/feed/v1/rss|atom`) uses `load_post_access_map` only (`feed_v1/mod.rs:165,234`) — reads
`post_access`, which exists — so the feed does **not** 500.

Release builds hide the DB detail (`error/response.rs:12-28`: `message`/`details` are
`cfg(debug_assertions)`), so production returns `{"type":"DB_004","status":500}`; dev returns the
table name in `details`.

### 1.3 The other live 500s

`GET /billing/v1/plans` is public (`billing_v1/mod.rs:62-64`) and reads `plans` → 500 **for anonymous
callers**. Every authenticated billing route (`/checkout`, `/checkout/post`, `/subscriptions`,
`/payments`) and every admin route 500s on a missing table before doing anything. `GET
/billing/v1/access/{post_id}` (`:64`) reads `post_access` and works. `POST /billing/v1/webhook/{provider}`
verifies the signature first and then fails at grant time (missing table → `QueryError` → 500 → PSP
retry loop).

---

## 2. Reachability verdict

| Caller                           | Route                                           | Result                                                                                               |
| -------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Anonymous                        | `POST /post/v1/list/published`                  | **200** — both lookups are skipped (`controller.rs:127-129`)                                         |
| Authenticated `User` (ordinary)  | `POST /post/v1/list/published`                  | **500**, any page, even empty list                                                                   |
| Authenticated `Author`           | `POST /post/v1/list/published`                  | **500**                                                                                              |
| `Moderator`/`Admin`/`SuperAdmin` | `POST /post/v1/list/published`                  | **200** (`is_moderator`, `user/model.rs:49-51`)                                                      |
| Authenticated `Author`           | `POST /post/v1/query`                           | **500**                                                                                              |
| Anonymous or authenticated       | `GET /billing/v1/plans`                         | **500**                                                                                              |
| Authenticated (any)              | `/billing/v1/{checkout,subscriptions,payments}` | **500**                                                                                              |
| Any viewer                       | `POST /post/v1/view/{slug}`                     | **200** while every policy is Free; 500 for non-bypass viewers once a Paid/SubscriberOnly row exists |
| Any viewer                       | `/feed/v1/rss`, `/feed/v1/atom`                 | **200**                                                                                              |

Nuances:

- The route is POST + CSRF (`middlewares/static_csrf.rs`), and the session cookie is `SameSite=Lax`
  (`main.rs:783`). A cross-site POST cannot carry the session, so the 500 is hit by same-origin
  authenticated callers (web-app JS with credentials, or any API client holding cookie + CSRF token).
- The shipped web UI never calls it: `rg 'post/v1' web/src` → 0 hits, and there is no billing/checkout/
  pricing page anywhere under `web/src/routes/`. So today no end user sees this in the product; the
  blog/mobile consumers of the API are the exposure, plus any future web caller.
- What the user sees is a 500 JSON error with no data (`{"type":"DB_004","status":500}` in release).
  Not an empty list, not a 403 — the whole request fails.

---

## 3. Provider audit

**Count.** `main.rs:241-416` registers **11** env-gated providers via `try_init`: `stripe`, `polar`,
`lemon_squeezy`, `paddle`, `crypto`, `crypto_multi`, `razorpay`, `mercado_pago`, `airwallex`,
`revolut`, `paypal` (10 source modules; `crypto` twice). C4's "9 providers" undercounts; the
`services/billing/README.md` table lists 5 and is stale. **No billing env var appears in
`.env.example` or `docker-compose.yml`** (the API container reads `.env` via `env_file`,
`docker-compose.yml:119`), so whether any provider is live in production is not determinable from the
repo. If none is set, every checkout returns 502 "Provider 'stripe' not initialized" — after the
`plans` lookup, which 500s first anyway.

**Signature verification.** Present for every provider; raw body bytes are used; comparisons are
constant-time (`ct_eq`, `crates/rux-webhook-crypto/src/lib.rs:21-27`); timestamp freshness (300 s,
`:6,36-38`) is enforced wherever the provider signs a timestamp.

| Provider              | Algorithm in code                                                                                                                                           | Correct vs published spec?                                                                                                                                                                                                                                                                                                         |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stripe                | HMAC-SHA256 over `t.body`, `Stripe-Signature: t=…,v1=…`, 300 s (`stripe.rs:262-306`)                                                                        | **Yes**                                                                                                                                                                                                                                                                                                                            |
| Polar                 | Standard Webhooks: HMAC-SHA256 over `{webhook-id}.{webhook-timestamp}.{body}`, base64, `v1,` prefix, 300 s (`polar.rs:188-199`; `rux-webhook-crypto:48-99`) | **Yes**                                                                                                                                                                                                                                                                                                                            |
| Lemon Squeezy         | HMAC-SHA256 over raw body, `X-Signature` hex (`lemon_squeezy.rs:209-222`)                                                                                   | **Yes**                                                                                                                                                                                                                                                                                                                            |
| Razorpay              | HMAC-SHA256 over raw body, `X-Razorpay-Signature` hex (`razorpay.rs:239-252`)                                                                               | **Yes**                                                                                                                                                                                                                                                                                                                            |
| Airwallex             | HMAC-SHA256 over `{x-timestamp}{body}` (ms normalized), `x-signature` hex (`airwallex.rs:218-253`)                                                          | **Yes**                                                                                                                                                                                                                                                                                                                            |
| Mercado Pago          | HMAC-SHA256 over `id:{data.id};request-id:{x-request-id};ts:{ts};`, with the alphanumeric-lowercase rule (`mercado_pago.rs:265-360`)                        | **Yes**                                                                                                                                                                                                                                                                                                                            |
| PayPal                | Calls PayPal `verify-webhook-signature` API with transmission headers; requires `PAYPAL_WEBHOOK_ID` (`paypal.rs:304-345`; `main.rs:389-402`)                | **Yes**                                                                                                                                                                                                                                                                                                                            |
| **Paddle**            | **Ed25519** over `{ts}{body}` (no separator), header field `key1=`, `PADDLE_PUBLIC_KEY` (`paddle.rs:246-296, 398-423`)                                      | **NO** — Paddle Billing signs **HMAC-SHA256 over `ts:body`**, header `ts=…;h1=…`, verified with the endpoint **secret key** (Paddle docs, `developer.paddle.com/webhooks/about/signature-verification`). A genuine event has `h1=`, so the code errors "Paddle-Signature missing key1=" and rejects **every real Paddle webhook**. |
| **Revolut**           | HMAC-SHA256 over `{ts}.{body}` (`revolut.rs:214-231`)                                                                                                       | **NO** — Revolut's signed payload is `v1.{Revolut-Request-Timestamp}.{raw body}`; the literal `v1.` prefix is required. Genuine Revolut events are rejected.                                                                                                                                                                       |
| Crypto / Crypto-multi | Rejects all webhooks fail-closed ("on-chain polling not implemented", `crypto.rs:155-161, 301-307`)                                                         | N/A (intentional)                                                                                                                                                                                                                                                                                                                  |

**What a forged webhook does.** `webhook_receiver` (`controller.rs:544-588`) verifies **before**
parsing or processing; a bad signature/missing header/unknown provider maps to
`ErrorCode::ExternalServiceError` → HTTP **502** (`ruxlog-types/src/error.rs:263`) with no processing,
no grant, no write. Fail-closed and safe; the status code is semantically wrong (an auth failure
should be 401/400), and unknown providers also 502.

**Grant integrity (positive finding).** Grants never trust client-shaped metadata: checkout binds a
server-side `CheckoutIntent` keyed by the PSP session id (`controller.rs:34-88`, `:421-429`, `:490-497`)
and the webhook consumes it single-use (`take`, `:77-87`); a webhook with no resolvable intent is
refused (`:658-670`, `:983-996`). Replay dedup is a body-hash claim-then-commit over the in-memory gate
store, released on processing failure (`:569-618`).

---

## 4. Abuse surface if billing goes live

Legend: **D** = defended in code today (tables absent means nothing is _reachable_; the column answers
"does the code defend it once the tables exist?").

| Vector                                                          | D                     | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Minimum fix                                                                                                                                                                                                                                              |
| --------------------------------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Card testing** — mass checkout creation to probe stolen cards | **No**                | `create_checkout`/`create_post_checkout` (`controller.rs:376-506`) have no per-user or per-account cap; the only limit is the nest layer 60/min/**IP**/matched-route (`router.rs:160-163`); `abuse_limiter` is used by auth/forgot/newsletter but **never by billing** (`rg abuse_limiter src/modules/billing_v1 src/services/billing` → 0). `payment_method_types[0]=card` (`stripe.rs:83`)                                                                                                                                  | Per-user velocity (`abuse_limiter` keyed `checkout:{user_id}`, e.g. 5/h + 20/day), per-IP/day cap, PSP Radar/fraud rules, 3DS on the PSP side                                                                                                            |
| **Discount-code brute force**                                   | **N/A (no endpoint)** | `discount_codes` is admin CRUD only (`:302-361`); `CreateCheckoutPayload` has no code field (`validator.rs:48-63`); no `redeem` anywhere; `redeemed_count` is set to 0 on create (`:328`) and never incremented; `max_redemptions` never enforced                                                                                                                                                                                                                                                                             | If a redemption endpoint is ever added: limiter keyed `discount:{user_id}`/IP, min code entropy (≥8 random chars), atomic redemption counter, unique constraint                                                                                          |
| **Refund / chargeback fraud**                                   | **Partial**           | `is_refund_or_dispute_event` covers 13 native types + Paddle `adjustment.*` action gating (`provider.rs:45-75`); `process_refund_or_dispute` deletes the post entitlement, flags the payment `Refunded`, cancels the subscription (`controller.rs:1088-1159`). But the post-entitlement revocation path is Stripe-shaped (`data.data.object.metadata.post_id`, `:1080-1086`) — only Stripe's per-post checkout sets that metadata; no dispute record, no ledger, no repeat-offender linkage; `payout_ledgers` is a dead model | Per-provider refund mapping with a test each; persist disputes; treat repeat chargebacks as a risk signal; use `provider_payment_id` + PSP card fingerprint (HMAC) for linkage                                                                           |
| **Webhook replay**                                              | **Mostly yes**        | Signature first; 300 s freshness where signed; body-hash dedup 24 h claim-then-commit (`:569-618`); single-use intent; idempotency checks on `post_purchases`/`subscriptions`/`payments`                                                                                                                                                                                                                                                                                                                                      | Residual: dedup is **in-memory** (lost on restart, per-instance), and the idempotency checks are read-then-write with **no unique constraints** (no migration). Add durable dedup + `UNIQUE(provider, provider_payment_id)` + `UNIQUE(user_id, post_id)` |
| **Price / amount tampering**                                    | **Yes**               | Client never sends an amount: `CreatePostCheckoutPayload` is `post_id`+URLs only (`validator.rs:56-63`, comment: "do NOT add a client-supplied amount field"); the price comes from `post_access.price_cents` (`controller.rs:453-462`); grants use the server-bound intent (`:679-680`, `:1010-1013`), never the client                                                                                                                                                                                                      | None. (Optional: assert the PSP-reported amount equals the intent before granting.)                                                                                                                                                                      |
| **IDOR on invoices/subscriptions**                              | **Yes**               | `my_subscriptions` filters `UserId = session user` (`:516-521`); `my_payments` likewise (`:534-539`); no user-facing invoice route exists; admin routes are role-gated (`mod.rs:51-53`)                                                                                                                                                                                                                                                                                                                                       | None for IDOR. Trim `metadata` (raw webhook JSON) out of user-facing DTOs                                                                                                                                                                                |
| **Provider-hosted open redirect**                               | **No**                | `success_url`/`cancel_url` are client-supplied strings with only a default (`controller.rs:397-402`, `:464-469`), passed verbatim to the PSP (`stripe.rs:94-96`, `:146-147`); validator has no URL check (`validator.rs:48-63`)                                                                                                                                                                                                                                                                                               | Fixed allowlist (same-origin paths, or `FRONTEND_URL`-prefixed absolute URLs)                                                                                                                                                                            |
| **Forged webhook**                                              | **Yes**               | See §3                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Map verification failure to 401/400 instead of 502                                                                                                                                                                                                       |

---

## 5. The correct v1 action

Options as posed:

- **(a) Add the missing migrations.** Smallest _table_ work (one `m000012` for 8 tables + constraints,
  ≈0.5-1 day), but it is the **unsafe** option: the moment `plans` exists, an admin can create a plan
  and `POST /billing/v1/checkout` returns a live PSP URL — with the open redirect, no card-testing
  control, two dead verifiers, unconstrained tables, and no refund mapping outside Stripe. D5 2c's
  own pre-live list is exactly this. **Rejected for v1.**
- **(b) Unmount `billing_v1`.** Correct, but **insufficient alone** — the 500 lives in `post_v1`'s
  paywall call, not in billing routes. It also removes the public `/billing/v1/plans` 500 and the dead
  webhook endpoint.
- **(c) Fix only the read path.** Sufficient for the availability bug (≈10 LOC), but leaves the
  module's other routes 500ing (public `/plans` anonymously) and leaves a half-dead payment surface
  mounted.

**Recommendation: (b) + (c).** Degrade the paywall list lookups (fail-closed on content, no 500) and
unmount `billing_v1`. Both are small, reversible, and keep the code + provider tests for the day the
module is re-enabled.

Test safety of unmounting — checked:

- `tests/quran_v1.rs:879-886` derives a route count, but **only** for `quran_v1`'s own `mod.rs`
  (`.matches(".route(")` over `include_str!("../src/modules/quran_v1/mod.rs")`). There is **no global
  route-count/CORS-table test**; `rg 'matches(".route(")' tests/ src/` → that one hit.
- `middlewares/static_csrf.rs:174-178` asserts `/billing/v1/webhook/*` is CSRF-exempt — those are
  string-level unit tests of `is_csrf_exempt`, not router mounts; they keep passing.
- `tests/billing_providers.rs` (1769 lines) exercises providers directly, not routes; unaffected.
- `tests/api_access_security_regressions.rs:178` nests `post_v1` and keeps `billing_router` in
  `AppState`; the router change does not touch `AppState`.
- `billing_v1` is referenced only at `router.rs:38` (import) and `router.rs:162` (the nest);
  `modules/mod.rs:24` keeps `pub mod billing_v1;` so the code still compiles.

Sketch of the paywall degrade (fail-closed: a lookup failure means "no purchases", so gated content
is stripped — never the reverse):

```rust
let (purchased, has_active_sub) = match viewer.filter(|_| !is_staff) {
    Some(u) => (
        paywall::user_purchased_post_ids(&state.sea_db, u.id, &ids)
            .await
            .unwrap_or_else(|e| { tracing::warn!(error = ?e, "purchase lookup failed; denying"); HashSet::new() }),
        paywall::user_has_active_subscription(&state.sea_db, u.id)
            .await
            .unwrap_or_else(|e| { tracing::warn!(error = ?e, "subscription lookup failed; denying"); false }),
    ),
    None => (HashSet::new(), false),
};
```

Plus one integration test: authenticated non-staff user → `POST /post/v1/list/published` → 200
(anonymous and staff variants already work by construction; no test covers any of them today).

---

## 6. Commercial status (DSA micro/small + consumer law)

**Does a mounted billing module make the service commercial? No — and the module cannot make it
commercial, because it cannot take money.**

- The legal test for DSA Art 19's micro/small exemption is Recommendation 2003/361/EC: <50 staff and
  (≤ €10m turnover or ≤ €10m balance sheet) for "small". That is a fact about the operator, not the
  code. Mounted-but-dead routes are not turnover.
- Code fact: **no payment can complete today.** Checkout 500s before reaching the PSP (`plans` missing,
  `controller.rs:387-395`); per-post checkout requires a `post_access` row that no route can create
  while billing is mounted-but-tables-absent for admin writes (admin `post/access` writes
  `post_access`, which exists — but there is no UI and no purchase path); webhooks cannot grant
  (missing tables); there is no checkout page in `web/`. A payment would have to be taken outside this
  codebase.
- What must be verified **with the owner** (the repo cannot answer it): (i) has any revenue ever been
  received through any channel (donations, sponsorship, external link)? (ii) is a paid plan intended
  in the next 12 months? (iii) are any PSP accounts live with this API's webhook URLs registered?
- If any revenue exists or is intended: assume commercial. Then re-run CCPA "business" thresholds
  (notice at collection, §7012), and the consumer-law set that R2/C4 leave out of scope: right of
  withdrawal for digital content (with consent + acknowledgment to lose it), refund policy, VAT/tax
  registration (EU OSS / merchant-of-record choice), ToS for paid services. DSA Art 17 statements of
  reasons stay owed either way (C4 §4.5; A5 F11). Art 20's complaint system is exempt only while
  micro/small and ends on growth.
- Do **not** cite "billing is mounted" as evidence of non-commerciality in any assessment; it is
  evidence of intent. A one-page written assessment with a review trigger ("re-check on first
  payment / first paid plan / annually") is the defence C4 asks for.

---

## 7. Privacy — what payment data is stored

**Nothing is stored today** (tables do not exist). Once the models' tables exist, the persisted fields
would be:

| Table                          | Persisted                                                                                                                                                                                                                                    | Card data?                                                      |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `payments`                     | `user_id`, `subscription_id`, `plan_id`, `provider`, `provider_payment_id` (PSP object id, e.g. `pi_…`), `amount_cents`, `currency`, `status`, `description`, **`metadata` = full raw webhook JSON** (`controller.rs:903-915`, `:1037-1052`) | No PAN/CVV columns. `metadata` can carry PSP-provided fragments |
| `subscriptions`                | `user_id`, `plan_id`, `provider`, `provider_customer_id`, `provider_subscription_id`, `status`, period timestamps, **`metadata` = raw webhook JSON** (`:766-791`)                                                                            | No                                                              |
| `invoices`                     | `user_id`, amounts, `status`, `due_date`, `paid_at`, `pdf_url`, `metadata`                                                                                                                                                                   | No                                                              |
| `post_purchases`               | `user_id`, `post_id`, `payment_id`, `provider`, amount, currency                                                                                                                                                                             | No                                                              |
| `discount_codes`               | admin-authored codes (plaintext), limits, `redeemed_count`                                                                                                                                                                                   | No                                                              |
| `payout_accounts` (dead model) | `provider_account_id` + `metadata` **encrypted envelope** `{"enc": …}` with auto-encrypt on write (`payout_account/model.rs:20-45`) — would hold IBAN/bank fields                                                                            | Bank data, encrypted; no card                                   |
| `payout_ledgers` (dead model)  | amounts, balance                                                                                                                                                                                                                             | No                                                              |

**R2-ban §6 / R2-evasion §6.4 rule** ("store a **keyed HMAC** of the payment instrument identifier
your PSP exposes — Stripe's `card.fingerprint`, Adyen's recurring detail reference; **never store or
hash the PAN yourself; never store the CVV**"):

- The code **does not violate "never store PAN/CVV"** — there is no card-number or CVV field anywhere
  in the models, and `provider_payment_id` is a PSP object id, not a PAN. It also does not implement
  the fingerprint-HMAC linkage (there is no `card.fingerprint` capture at all), so the multi-account
  payment-linkage lever R2 describes does not exist.
- The concrete privacy defect: `metadata` is a **verbatim dump of the webhook payload** with no schema
  allowlist and no retention class. Mercado Pago's payment resource includes a `card` object
  (`first_six_digits`, `last_four_digits`, `cardholder` name/identification) and the code stores
  `{"type":…, "data": resource}` (`mercado_pago.rs:452`); PayPal payloads carry payer email/name.
  Stored raw, that is cardholder + payer PII with no expiry and no disclosure. Fix: field allowlist
  (ids, amounts, status only), never the `card`/`payer` objects; retention class per C4 §3
  (tax-law 7-10 years for financial rows; drop the raw payload at 90 days).

---

## 8. Minimum safe v1 action, estimate, preconditions

**Do now (2-4 h total):**

1. Degrade the two paywall list lookups (fail-closed) + add the authenticated-list integration test —
   ≈10-15 LOC + ~80 LOC test.
2. Unmount `billing_v1` (remove `router.rs:38` import and the `router.rs:160-163` nest; keep the
   module and provider tests). One line of comment pointing at this report.
3. No migration in this change. No UI change. No test expected to break (verified in §5).

**Before any payment can be accepted (the re-enable checklist):**

1. `m000012` for 8 tables with `UNIQUE(provider, provider_payment_id)`,
   `UNIQUE(user_id, post_id)`, `UNIQUE(discount_codes.code)`, INTEGER-epoch timestamps (C4 §3 note).
2. Fix Paddle (HMAC `h1` over `ts:body`) and Revolut (`v1.` prefix) verifiers, each with an
   independent-signer test.
3. Redirect allowlist for `success_url`/`cancel_url`.
4. Checkout velocity limiter (per-user + per-IP/day) and a written card-testing posture (PSP fraud
   rules, 3DS).
5. Refund/dispute mapping per provider + a dispute record.
6. Durable webhook dedup (or rely on the new unique constraints) and a `WebhookVerification → 401`
   mapping.
7. `metadata` allowlist + retention class; no `card`/`payer` objects.
8. Commercial/consumer-law docs from §6 (withdrawal, refunds, tax/VAT, paid-service ToS) and the
   privacy-policy update for paid users.
9. Live PSP account + webhook secrets, documented in `.env.example` (currently absent), plus the
   unmount reversal as its own reviewed commit.

---

## 9. Findings (12, all NEW relative to D5/A5/C4/C3)

### F1 — Eight of nine billing tables have no migration; the paywall read path 500s the authenticated post feed

**Severity: High (availability). NEW.**
Evidence: `migration/src/lib.rs:3-14` (stops at m000011); `rg 'post_purchases|subscriptions|payments|invoices|discount_codes|plans|payout' migration/src/` → 0; dev DB `.tables` lacks all eight; `services/paywall.rs:115-134` (`post_purchases`) and `:136-161` (`subscriptions`); `post_v1/controller.rs:122-126` (both queries for non-staff viewers), call at `:330`; `error/database.rs:4-46` → `QueryError` → 500 (`ruxlog-types/src/error.rs:247`).
Fix: §8 steps 1-2 (degrade + unmount); migrate only as part of the re-enable checklist.

### F2 — `user_has_active_subscription` has no empty-input early return: the 500 fires even for an empty list

**Severity: High (makes F1 unconditional). NEW.**
Evidence: `paywall.rs:121-123` (purchase lookup early-returns on empty ids) vs `:136-147` (subscription lookup always queries); `post_v1/controller.rs:122-126`.
Fix: same as F1; the degraded path must not depend on list length.

### F3 — Public `GET /billing/v1/plans` 500s anonymously; every billing route 500s

**Severity: High (public 500 surface). NEW.**
Evidence: `billing_v1/mod.rs:62-64` (public router), `controller.rs:363-374` (`plans` query); admin/auth routes `mod.rs:13-60`; webhook `:65`.
Fix: unmount until tables + checklist; if kept, return 503 for a not-provisioned module.

### F4 — Paddle webhook verifier implements a scheme Paddle does not use; every genuine Paddle event is rejected

**Severity: Medium (provider dead; fail-closed, not a vuln). NEW.**
Evidence: `paddle.rs:246-296` reads `key1=` and verifies **Ed25519 over `{ts}{body}`** with `PADDLE_PUBLIC_KEY`; `main.rs:305-322`. Paddle Billing signs **HMAC-SHA256 over `ts:body`** with the endpoint secret key and sends `ts=…;h1=…` (`developer.paddle.com/webhooks/about/signature-verification`, fetched 2026-10-12). The unit test mirrors the impl's own signer (`paddle.rs:398-423`), so it cannot catch this.
Fix: HMAC-SHA256 `{ts}:{raw_body}`, parse `h1` (accept multiple during rotation), keep 300 s freshness; add a test vector from Paddle's docs.

### F5 — Revolut verifier omits the `v1.` prefix; every genuine Revolut event is rejected

**Severity: Medium (provider dead; fail-closed). NEW.**
Evidence: `revolut.rs:214-221` builds `{ts}.{body}`; Revolut's signed payload is `v1.{Revolut-Request-Timestamp}.{raw body}` (Revolut docs; Hookdeck `revolut-webhooks` reference, fetched 2026-10-12). Header parsing and 300 s freshness are otherwise correct (`:180-212`).
Fix: prepend the literal `v1.`; add a vector test.

### F6 — No card-testing control: checkout has no per-user cap; only a 60/min/IP nest limiter

**Severity: High (the day it goes live). NEW.**
Evidence: `router.rs:160-163` (nest 60/60, `PathKey::Matched`); `controller.rs:376-506` (no limiter call); `rg abuse_limiter src/modules/billing_v1 src/services/billing` → 0; `stripe.rs:83` card-only.
Fix: per-user + per-IP/day velocity via `abuse_limiter`, PSP fraud rules, 3DS; document the threshold.

### F7 — Client-supplied `success_url`/`cancel_url` are passed to the PSP (provider-hosted open redirect)

**Severity: Medium-High (the day it goes live). NEW.**
Evidence: `validator.rs:48-63` (no URL validation); `controller.rs:397-402`, `:464-469`; `stripe.rs:94-96`, `:146-147`; `paddle.rs`/others pass them through too.
Fix: fixed allowlist (relative paths resolved against `FRONTEND_URL`, or exact-match origin allowlist).

### F8 — Raw webhook payloads are persisted in `metadata` with no allowlist and no retention

**Severity: Medium (privacy; live only with tables). NEW.**
Evidence: `controller.rs:789`, `:913`, `:1050` (`metadata: Set(Some(event.data.clone()))`); `mercado_pago.rs:452` stores the fetched payment resource (`card.first_six_digits`/`last_four_digits`/cardholder); PayPal resources carry payer PII. No PAN/CVV columns exist (good), but no fingerprint-HMAC either (R2-evasion §6.4 unimplemented).
Fix: allowlist metadata to ids/amounts/status; 90-day raw-payload retention; never persist `card`/`payer` objects; if linkage is wanted, HMAC the PSP fingerprint.

### F9 — Refund/dispute revocation is Stripe-shaped and incomplete for other providers; no dispute record

**Severity: Medium (latent). NEW.**
Evidence: `controller.rs:1080-1086` reads `data.data.object.metadata.post_id`; only Stripe's `create_post_checkout` sets that metadata (`stripe.rs:148-149`); `provider.rs:45-75` lists the 13 native refund types across providers; `payout_ledgers` is a dead model (zero refs).
Fix: per-provider refund target mapping with a test each; persist disputes; link repeat chargebacks.

### F10 — Webhook dedup is in-memory only and idempotency has no unique constraints

**Severity: Low-Medium (latent). NEW.**
Evidence: `controller.rs:569-618` (dedup over `gate_store`, 24 h, lost on restart / per instance); the idempotency checks at `:682-695`, `:733-750`, `:886-901`, `:1020-1035` are read-then-write; the models declare no unique index and no migration exists.
Fix: durable dedup (or the `UNIQUE` constraints in §8.1); replay is otherwise well-defended (single-use intent + freshness).

### F11 — Forged webhooks return 502; unknown providers return 502

**Severity: Low (correctness/observability). NEW.**
Evidence: `controller.rs:560-567` maps `BillingError::WebhookVerification` to `ExternalServiceError`; `ruxlog-types/src/error.rs:263` → 502.
Fix: 401/400 for verification failures; keep 502 for genuine upstream errors.

### F12 — The module is API-only: no billing UI in `web/`, no billing env vars documented, and `plans`/`payout_*`/discount redemption are dead code

**Severity: Informational (scope/commercial-status input). NEW.**
Evidence: `rg 'billing|checkout' web/src/routes` → 0 (only two unrelated copy files); no `post/v1` caller in `web/src`; `.env.example` and `docker-compose.yml` contain no PSP variables (API reads `.env` via `env_file`, `docker-compose.yml:119`); `payout_account`/`payout_ledger` have zero refs outside their models; `discount_codes.redeemed_count` never increments (`controller.rs:328`).
Consequence: nothing user-reachable can take money today; provider liveness in production must be answered by the owner (§6), not inferred from the repo.

---

## Appendix — verification commands (read-only)

```bash
# missing tables
cd rust/backend/api
rg -n "post_purchases|subscriptions|payments|invoices|discount_codes|plans|payout" migration/src/   # 0
sqlite3 "file:../../data/easyquran.db?immutable=1" ".tables"                                        # no billing tables
sqlite3 "file:../../data/easyquran.db?immutable=1" 'SELECT "post_purchases"."id" FROM "post_purchases" WHERE "post_purchases"."user_id"=1 AND "post_purchases"."post_id" IN (1,2);'
sqlite3 "file:../../data/easyquran.db?immutable=1" 'SELECT "subscriptions"."id" FROM "subscriptions" WHERE "subscriptions"."user_id"=1;'

# the call path
sed -n '113,152p' src/modules/post_v1/controller.rs
sed -n '115,161p' src/services/paywall.rs
sed -n '314,343p;399,443p' src/modules/post_v1/controller.rs

# mount + limit + test safety
sed -n '160,163p' src/router.rs
rg -n 'matches\(".route\("' tests/ src/                     # quran_v1 only
rg -n "/billing" src/middlewares/static_csrf.rs tests/

# providers + verification
grep -c 'try_init(' src/main.rs                              # 11
rg -n "fn verify_webhook" src/services/billing/
sed -n '246,296p' src/services/billing/paddle.rs             # Ed25519 + key1 (wrong scheme)
sed -n '214,231p' src/services/billing/revolut.rs            # missing v1. prefix
sed -n '10,46p;48,99p' crates/rux-webhook-crypto/src/lib.rs  # HMAC/ct_eq/freshness/standard-webhooks

# abuse surface
rg -n "abuse_limiter" src/modules/billing_v1 src/services/billing   # 0
sed -n '397,402p;464,469p' src/modules/billing_v1/controller.rs     # client URLs
sed -n '1080,1159p' src/modules/billing_v1/controller.rs            # refund/dispute

# models (no PAN/CVV)
rg -n "card|pan|cvv" src/db/sea_models/{payment,invoice,subscription,plan,discount_code,post_purchase}/model.rs   # 0
sed -n '1,45p' src/db/sea_models/payout_account/model.rs             # encrypted envelope

# web
rg -rn "billing|checkout" web/src/routes | head
```
