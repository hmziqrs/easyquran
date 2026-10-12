# A9 — Cryptography & Key Management Audit

**Inputs audited:** `DESIGN-v0.md` (Layers 0–6), `R2-session-binding-rba.md` §8 (keyed HMAC, rotation, mapping-table indirection, deletion), §9.3 (codebase notes).
**Method:** every repo claim below was read from source, not inferred. Citations are `file:line`.
**Scope:** key hierarchy, rotation, device token, edge header signing, evidence integrity, deletion/erasure, failure modes, v1 minimum.

---

## 0. Repo facts this audit rests on (verified)

| Fact                                                                                                                                                                                                                         | Evidence                                                                                                                           |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `COOKIE_KEY` is required, ≥32 bytes, placeholder-blocklisted                                                                                                                                                                 | `src/config/settings.rs:886-888`, `src/state.rs:74-106`                                                                            |
| `COOKIE_KEY` → tower-sessions cookie private key (HKDF)                                                                                                                                                                      | `src/main.rs:776-788` (`Key::derive_from`)                                                                                         |
| `COOKIE_KEY` → CSRF signing key via local HKDF-SHA256, info `ruxlog-csrf-v1`                                                                                                                                                 | `src/middlewares/static_csrf.rs:17-47`                                                                                             |
| CSRF key stored as `OnceLock<Vec<u8>>` — **not** zeroized                                                                                                                                                                    | `src/middlewares/static_csrf.rs:15`                                                                                                |
| `FIELD_ENC_KEY` is a **separate** 32-byte secret, AES-256-GCM, prod panic on unset/dev-default, `_PREV` decrypt-only rotation slot, `Zeroizing` slots, mid-process change rejected                                           | `src/state.rs:108-179`, `src/utils/field_crypto.rs:13-66,112-114`                                                                  |
| `INTERNAL_QURAN_API_TOKEN` is a third separate secret, prod length + placeholder gates, compared constant-time, present-but-invalid falls through (denies internal treatment)                                                | `src/config/settings.rs:456-504`, `src/middlewares/client_ip.rs:89-106`                                                            |
| `IP_SOURCE=ConnectInfo` rejected in prod; `CF-Connecting-IP` required in prod                                                                                                                                                | `src/config/settings.rs:208-215`, `docker-compose.yml:136`                                                                         |
| Internal token handed to middleware as `Extension(Arc<str>)` from boot settings                                                                                                                                              | `src/main.rs:824-835`                                                                                                              |
| HMAC/verify toolkit exists: `hmac_sha256_hex`, `verify_hmac_sha256_hex`, `ct_eq`, `header_str` (`.get()` = **first value wins**), `timestamp_fresh`, `MAX_SKEW_SECS = 300`, `verify_ed25519`                                 | `crates/rux-webhook-crypto/src/lib.rs:6-110`                                                                                       |
| Turnstile fails **closed** on unconfigured-in-prod and on transport failure                                                                                                                                                  | `src/services/turnstile.rs:42-47,203-209`                                                                                          |
| SQLite only, **pool = 1 connection**, migrations auto-run at boot                                                                                                                                                            | `src/db/sea_connect.rs:32-37,89`                                                                                                   |
| **One** `api` container, `APP_ENV: production` hardcoded, Traefik only middleware is `stripprefix /api` (no header stripping)                                                                                                | `docker-compose.yml:113-193`, esp. `:132,:189-192`                                                                                 |
| `audit_logs` table exists, zero writers, `user_id` FK `ON DELETE SET NULL`                                                                                                                                                   | `migration/src/m000001_init.rs:252-264`                                                                                            |
| Migrations stop at m000011; **`devices` already means FCM push tokens** (user_id, token, platform), unique token owner enforced                                                                                              | `migration/src/lib.rs:12-17`, `m000005_device_notification_oauth.rs:13-26`, `m000011_device_token_ownership.rs:11-21`              |
| `BanUnit` is **IP-only** by construction: v4→/32, v6→/64, `parse()` accepts only those widths                                                                                                                                | `src/modules/admin_bans_v1/dto.rs:16-43`                                                                                           |
| Ban export feed ships `banUnit` strings to a deployment-owned proxy adapter outside the repo                                                                                                                                 | `src/modules/admin_bans_v1/dto.rs:90-100`, `mod.rs:16-29`, `deploy/README.md:419-429`                                              |
| CF→Traefik→api; locking origin ingress to Cloudflare ranges is an owner-run step (`deploy/README.md:377-400`, unverified), and the api is still reachable from any container on the shared Docker network (F1-ingress-trust) | `deploy/README.md:377-400`, `docker-compose.yml:181-192`                                                                           |
| `X-Eq-Edge` appears **nowhere** in the repo; no Worker code, no wrangler config                                                                                                                                              | `rg X-Eq-Edge` → 0 hits; no `wrangler*` file                                                                                       |
| Scheduler exists for periodic jobs (post publisher, interval tick)                                                                                                                                                           | `src/main.rs:761`, `src/services/scheduler.rs:11-20`                                                                               |
| IDB_VERSION pinned at 1, one-store-per-DB invariant → a new store needs a **new DB name**                                                                                                                                    | `web/src/lib/workers/idb.ts:2,19-31`                                                                                               |
| localStorage conventions live in safe-storage (try/catch, JSON)                                                                                                                                                              | `web/src/lib/storage/safe-storage.ts`                                                                                              |
| Session auth hash exists and is compared constant-time                                                                                                                                                                       | `crates/rux-auth/src/session/extractor.rs:19-27,53`                                                                                |
| sha guards (see §5)                                                                                                                                                                                                          | `tests/quran_v1.rs:925-951,1237-1275,1665-1691`; `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts:11-51`; `justfile:31-32` |

---

## 1. Key hierarchy

### 1.1 What this feature actually needs

| #   | Key                                    | Purpose                                            | Lifetime / rotatable?                                                                                                              |
| --- | -------------------------------------- | -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| K1  | `DEVICE_KEY_PEPPER` (new secret, 32 B) | IKM for two HKDF subkeys                           | Long-lived; rotatable **only** with a documented invalidation event (see §2)                                                       |
| K1a | `HMAC(K1, "ruxlog-device-token-v1")`   | hash the server-minted device token before storage | **Effectively non-rotatable** — see §1.4                                                                                           |
| K1b | `HMAC(K1, "ruxlog-device-fp-v1")`      | hash fingerprint components → `fp_digest`          | Freely rotatable with dual-key read                                                                                                |
| K2  | `DEVICE_KEY_PEPPER_PREV` (optional)    | decrypt-only/read-only previous slot               | Exists only during a rotation window                                                                                               |
| K3  | Edge attestation key                   | sign edge claims at the Cloudflare edge            | Rotatable by kid; no stored data depends on it                                                                                     |
| K4  | Evidence anchor key                    | sign daily evidence roots (if built)               | Rotatable by kid; old roots verify under old public keys                                                                           |
| —   | Encryption key                         | **none new**                                       | If evidence ever carries PII, reuse `FIELD_ENC_KEY` (`field_crypto::encrypt`, AES-256-GCM, already installed at `src/main.rs:166`) |

No new encryption key. Evidence is either non-PII or encrypted with the existing field key. **Do not** add a fourth symmetric secret for evidence.

### 1.2 Reuse HKDF-from-`COOKIE_KEY`, or a new secret? — **New secret. Argued explicitly.**

**For reusing `COOKIE_KEY`:** one fewer env var, one fewer boot gate, and the repo already derives CSRF from it (`static_csrf.rs:45`), so the pattern is established and cheap.

**Against — three concrete reasons, in order of weight:**

1. **Blast-radius increment is real and avoidable.** Today a `COOKIE_KEY` leak yields: the tower-sessions cookie private key (`main.rs:777`) → forge/decrypt any session cookie → **full impersonation of any user**; plus the CSRF signing key → CSRF bypass. That is already game over. HKDF'ing the device pepper from it _adds_: the ability to compute `HMAC(pepper, class||version||components)` for any chosen component set → fabricate device-record matches, evade a `BanUnit::Device` ban, and forge the `fp_digest` of a "known" device. It also adds the ability to compute `token_hash` for a **chosen** token — which only helps if the token is guessable (it is 128-bit random, so this is mostly a non-issue) — but it does _not_ unlock stored token hashes for inversion. So the increment is "device-record forgery and ban evasion", which is strictly less than the session compromise, but it is a **new, permanent capability for the same single secret**. There is no reason to grant it.
2. **Rotation coupling.** A `COOKIE_KEY` rotation is a session-invalidating, all-users-re-auth event. The device pepper must be rotatable on its own schedule (or, for the token subkey, not at all). Deriving one from the other forces the wrong coupling: rotating the device pepper would require rotating `COOKIE_KEY`.
3. **Repo convention already says separate.** The repo has _three_ independent secrets for three different kinds of purpose: signing/session (`COOKIE_KEY`), encryption (`FIELD_ENC_KEY`), service auth (`INTERNAL_QURAN_API_TOKEN`) — each with its own prod gate and its own rotation story (`state.rs:108-179`, `settings.rs:456-504`, `.env.example:3-5`). R2 §8.4 states the same rule from the standards side: "Keep _identification_ keys (HMAC) distinct from _authentication_ secrets". A device pepper is identification. It belongs in its own slot.

**Construction:** add `DEVICE_KEY_PEPPER` (32 raw bytes, mirroring `FIELD_ENC_KEY`'s dev-default + prod-panic + placeholder-blocklist discipline at `state.rs:108-179`), then derive both subkeys with the **existing** HKDF helper shape from `static_csrf.rs:17-30` and distinct info labels:

```rust
// src/services/device_keys.rs (or src/utils/device_crypto.rs)
const INFO_TOKEN: &[u8] = b"ruxlog-device-token-v1";
const INFO_FP:    &[u8] = b"ruxlog-device-fp-v1";
fn token_subkey(ikm: &[u8]) -> Zeroizing<[u8;32]> { hkdf_sha256(ikm, INFO_TOKEN) }
fn fp_subkey(ikm: &[u8])    -> Zeroizing<[u8;32]> { hkdf_sha256(ikm, INFO_FP) }
```

Two subkeys, one secret. Store them in `OnceLock<Zeroizing<[u8;32]>>` (the `field_crypto` pattern, `field_crypto.rs:13-15`), **not** `OnceLock<Vec<u8>>` (the `static_csrf` pattern at `static_csrf.rs:15` leaks key material on process exit — a pre-existing minor issue, do not copy it into new code).

**Domain separation is mandatory and cheap:** the same raw component string must never produce the same digest in two classes. Bind purpose + version into the MAC input: `HMAC(k, b"fp:v1:" || canonical)` and `HMAC(k, b"tok:v1:" || token)`. R2 §8.1/§8.4; RFC 9415 §7.3.

### 1.3 Blast radius if `COOKIE_KEY` leaks — answer with the recommendation in place

| Leaked secret       | Attacker gains                                                                                                                    |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `COOKIE_KEY`        | Session cookie forgery for any user; CSRF bypass. **Not** the device pepper.                                                      |
| `DEVICE_KEY_PEPPER` | Ability to compute `token_hash`/`fp_digest` for chosen inputs → fabricate/evade device records and bans. **Not** session or CSRF. |
| Both                | Both, independently — but they are two distinct incidents with two distinct rotation procedures.                                  |

That is the point of the split: the fingerprint pepper does **not** unlock the ability to forge device _tokens_ in the sense of minting a valid token for someone else's record (you still need a 128-bit random token whose hash matches a stored row), and it does not touch sessions. With the HKDF-from-`COOKIE_KEY` alternative, one leak grants all three capabilities at once and one rotation event is required to fix everything.

### 1.4 The non-obvious constraint: the token subkey cannot be rotated

The server stores only `token_hash = HMAC(token_pepper, token)`. It never stores the token. Therefore it **cannot** re-hash existing rows under a new pepper — hashes are non-invertible. Consequences:

- Rotating `token_pepper` **permanently orphans every existing device record**: every returning browser presents a token whose hash matches nothing → every user is "new device" → a global step-up storm, and every active `BanUnit::Device` stops matching.
- Therefore the token subkey's rotation must be treated as a **deliberate, global invalidation event** (post-breach, "we believe the pepper leaked"), not hygiene. Store a `token_key_id` on each row so the event is diagnosable and so a mixed-key deployment is visible rather than silent.
- The fingerprint subkey is the opposite: `fp_digest` is recomputable from the same components on the next request, so it _can_ be rotated with a dual-key read + lazy rewrite.

Write this asymmetry down in the design; the current design text implies all keys are uniformly "rotatable" and that is false for K1a.

---

## 2. Rotation: audit of `HMAC(epoch_key, device_id)` weekly + mapping table

### 2.1 What it costs in this repo

| Cost    | Reality here                                                                                                                                                                                      |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rows    | With history: one mapping row per device per epoch ≈ 52/device/year. Without history: the mapping table is just an indirection to a stable pseudonym — which `device_records.id` already is.      |
| Lookups | An extra indexed lookup per device match, on a **pool = 1 connection** SQLite DB (`sea_connect.rs:35-36`). Every added query is serialized against everything else.                               |
| Jobs    | An epoch-key store + a rotation job. The scheduler exists (`main.rs:761`, `scheduler.rs:11-20`) but runs **process-local**; a rotation that fails silently leaves correlators under a dead epoch. |
| Window  | A dual-key read window for the epoch boundary, on top of the dual-key window you already need for the pepper. Two overlapping dual-key schemes.                                                   |
| Egress  | If the correlator is exported anywhere, every epoch boundary forces a re-export or the consumer's rules go stale.                                                                                 |

### 2.2 Does it help here? — **No. It is worse than nothing for the stated goal.**

The design's stated goals are (a) effective bans and (b) not ad tracking. Evaluate rotation against those:

- **For banning it actively hurts.** A ban must recognize a returning banned device across the boundary. Either (i) you keep the mapping table with a stable pseudonym → the stable pseudonym **is** the join that rotation exists to break, so rotation buys zero unlinkability; or (ii) you rotate without a stable pseudonym → a banned device becomes unrecognizable at the boundary → **ban evasion by waiting a week**. The mapping-table design is (i): it is stable by construction, so the rotation is cosmetic.
- **For privacy it buys nothing here.** The CDI draft pattern (R2 §8.3) exists because a _RADIUS intermediary_ sees the identifier. In this repo the only consumer of the digest is the same process that holds the key and the database. The digest never leaves the app DB — **except** through one channel the design misses (see §2.3).
- **Against the "public epoch label" trap:** R2 §8.3 already warns that per-epoch keys must be random and stored, which means a stored epoch-key table is itself a re-identification capability that must be protected and expired like PII (R2 §8.5). That is pure added retention surface for a property nobody consumes.

**Verdict:** drop weekly epoch rotation and the mapping table. Keep a stable internal `device_records.id` (ULID) as the durable identity, expose nothing, and make `fp_digest` a **versioned, dual-key-rotatable signal column** rather than a rotating identity. This is exactly R2 §8.5's "decouple the stable internal key from the exposed value" — with the exposure being nothing at all in v1.

### 2.3 The one egress the design forgot

The design proposes `BanUnit::Device(device_key)` alongside `/32` and `/64`. `BanUnit` is currently IP-only and its `canonical()` string is serialized into the **export feed** consumed by a deployment-owned proxy adapter outside this repo (`dto.rs:40-51,90-100`; `admin_bans_v1/mod.rs:16-29`; `deploy/README.md:419-429`). Adding a device axis therefore ships the pepper digest to an external system:

- the digest becomes **unrecallable** (an erasure request cannot retract it),
- epoch rotation would require re-exporting every epoch or the adapter's rules rot,
- and `BanUnit::parse`'s documented invariant ("an unparseable or wrong-width store key can never pose as an exportable ban unit", `dto.rs:13-15`) is deliberately broken.

**Correction:** keep the export feed IP-only. If a device axis is ever needed for the adapter, export a per-deployment opaque reference with an explicit `expiresAt` — never the pepper digest.

### 2.4 Concrete schemas

**Recommended (simpler) — the device migration (next free number), `device_records` (name avoids the existing `devices` = FCM push tokens, `m000005:13-23`):**

```sql
CREATE TABLE IF NOT EXISTS device_records (
    id            TEXT    PRIMARY KEY,            -- ULID, stable internal identity, never exposed
    token_hash    BLOB    NOT NULL,               -- HMAC(token_subkey, "tok:v1:" || token)
    token_key_id  INTEGER NOT NULL DEFAULT 1,     -- which token pepper wrote token_hash
    fp_digest     BLOB,                           -- HMAC(fp_subkey, "fp:v1:" || canonical components)
    fp_key_id     INTEGER,                        -- which fp pepper wrote fp_digest
    fp_version    INTEGER NOT NULL DEFAULT 0,     -- fingerprint schema version (self-describing)
    user_id       INTEGER REFERENCES users(id) ON UPDATE CASCADE ON DELETE SET NULL,
    trust_state   TEXT    NOT NULL DEFAULT 'new', -- new|known|trusted|suspect
    risk_score    INTEGER,
    coherence     TEXT,                           -- canonical JSON, coherence booleans ONLY (no raw signals)
    first_seen    TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen     TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    seen_count    INTEGER NOT NULL DEFAULT 1,
    revoked_at    TEXT,
    created_at    TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_token ON device_records (token_hash);
CREATE INDEX IF NOT EXISTS idx_device_records_user  ON device_records (user_id);
CREATE INDEX IF NOT EXISTS idx_device_records_fp    ON device_records (fp_digest);
CREATE INDEX IF NOT EXISTS idx_device_records_last  ON device_records (last_seen);

CREATE TABLE IF NOT EXISTS device_sessions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    device_record_id TEXT    NOT NULL REFERENCES device_records(id) ON UPDATE CASCADE ON DELETE CASCADE,
 user_session_id INTEGER NOT NULL, -- no FK: a CASCADE from user_sessions would erase the device↔account link when an account is deleted (A3 §5)
    created_at       TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_sessions_pair
    ON device_sessions (device_record_id, user_session_id);
```

Deletion = `DELETE FROM device_records WHERE id = ?` (cascades `device_sessions`; account deletion does **not** touch them). Rotation = bump `fp_key_id`, write new digests, dual-key read until old rows age out; token pepper rotation is the documented global invalidation event from §1.4.

**Rotation-capable alternative (documented, NOT recommended) — epoch keys + mapping table:**

```sql
CREATE TABLE IF NOT EXISTS device_epoch_keys (
    epoch      INTEGER PRIMARY KEY,
    key        BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS device_correlators (
    epoch            INTEGER NOT NULL,
    correlator       BLOB    NOT NULL,   -- HMAC(epoch_key, device_record_id)
    device_record_id TEXT    NOT NULL REFERENCES device_records(id) ON DELETE CASCADE,
    PRIMARY KEY (epoch, correlator)
);
```

Costs as tabulated in §2.1; benefit: none in this repo (§2.2). Recommendation: **schema 1**.

---

## 3. Device token design

### 3.1 What the design says, and what it does not

`DESIGN-v0.md:34`: "A first-party device token: server-minted opaque id, stored in localStorage + IDB". `DESIGN-v0.md:40-41` uses `device_key` as a ban unit. The design never states **what possession of the token proves**. That omission is the whole security question.

### 3.2 Theft model: what does copying the token buy?

- If token presence can assert "known device" and thereby **skip step-up**, then reading localStorage is an ATO enabler: attacker gets credentials (stuffing/phishing) + copies the token off a compromised browser profile/machine/XSS → never challenged. This is the DBSC problem in reverse: DBSC survives cookie theft because the key is non-exportable hardware (`R2 §2.2-2.3`); a localStorage string has no such property, and `R2 §10` says plainly that every binding is defeated by code execution on the bound device.
- If the token only feeds the **risk score**, theft buys a score contribution and nothing else — the attacker still faces IP/ASN/credential/behaviour signals and step-up.
- NIST's normative position (`R2 §5.3`): session secrets "SHOULD NOT be stored in insecure locations such as HTML5 Local Storage". So the token must not be a session secret, and must not be accepted as proof of possession.

**Correct model:** the device token is a **bearer identifier for a device record**. It answers "which record is this browser claiming?" — never "is this the same machine?" or "is this the legitimate user?". All trust is earned server-side (network, credential, behaviour, age, corroboration). The token never gates a decision; it _contributes_ to one. If the design ever wants the token to gate step-up, that is a design change requiring hardware-backed PoP (DBSC or WebAuthn), not a token change.

### 3.3 Should the client contribute entropy? — **No, at v1.**

Arguments for: it defeats partial leaks (a leaked server-side id list or a DB read of ids doesn't let you impersonate), and it is a cheap "poor-man's PoP".

Arguments against, which win here:

- The dominant threat is code execution in the origin (XSS) or on the device (infostealer, CDP). Both read **all** of localStorage, so both halves leak together; `R2 §1.4` and `§10` document this failure mode explicitly for browser-held keys.
- A client-held secret that can be cleared by site-data clearing or ITP creates a "new device" step-up storm and support load — a real, recurring cost.
- Without hardware binding it is theater: a server-side secret that lives next to the token in the same storage is not a possession factor.

If a binding signal is wanted, use the **existing** session auth hash (`rux-auth/src/session/extractor.rs:19-27,53`): record the `session_auth_hash` at device-record creation and flag a token presented under a different authenticated session's hash. That is a _signal_, not a gate, and it costs one column.

### 3.4 Signed (HMAC) or random-only? — **Random-only.**

The only verifier is the same process that holds the database. A signed token adds a key, a key id, an expiry, and a rotation story, and buys nothing that the DB lookup does not already give. Signing matters only if a **different** service must verify statelessly — not the case here.

What the token _does_ need:

1. **≥128 bits from the OS CSPRNG** (`getrandom`), never a counter/ULID as a bearer value. Mirror the nonce discipline: an RNG failure must propagate, never fall back (`field_crypto.rs:112-114`).
2. **Stored hashed**: `token_hash = HMAC(token_subkey, "tok:v1:" || token)`, `UNIQUE`. A DB read then yields no usable token.
3. **Revocable**: `revoked_at` + a device list/revoke UI; rotate on suspicious events.
4. **Never logged, never echoed in a response, never in a URL.**
5. **One storage location.** The design says localStorage **+** IDB. Two copies = two theft surfaces and a sync bug class; IDB adds nothing for a bearer token. Pick localStorage via the existing `readRaw`/`writeRaw` (`web/src/lib/storage/safe-storage.ts`). If IDB is wanted, it needs a **new DB name** — the pinned `IDB_VERSION = 1` plus the one-store-per-DB invariant means adding a store to an existing DB is a hard error (`web/src/lib/workers/idb.ts:2,19-31`).
6. **A per-user cap** and "revoke all devices" (the design's §L5 UI can host it).

---

## 4. Edge header signing — audit and corrected construction

`DESIGN-v0.md:54`: "Cloudflare Worker adds signed `X-Eq-Edge-*` headers (request.cf fields, JA4 only when the plan exposes it), HMAC-SHA256 + timestamp + kid, +/-30s replay window; origin strips client-supplied `X-Eq-Edge-*` and verifies."

### 4.1 Flaws, in order of severity

1. **No canonical string is specified → canonicalization ambiguity.** "HMAC-SHA256 over canonical string" with an unspecified string is unimplementable safely: delimiters inside field values, field ordering, and empty-vs-absent all become forgery surface. This is flaw #1 because every other flaw is amplified by it.
2. **Sign-the-map bug.** If the MAC covers "the `X-Eq-Edge-*` headers present", the _set_ is attacker-influenced: the attacker omits a field the origin then reads as absent, or adds a field the origin reads as unsigned. The set must be closed and fixed.
3. **Duplicate-header / smuggling.** `HeaderMap::get` returns the **first** value (`rux-webhook-crypto/src/lib.rs:29-34`). Traefik forwards unknown headers and its only api-router middleware is `stripprefix /api` (`docker-compose.yml:189-192`) — **no header stripping exists anywhere in the stack**. So a client can send its own `X-Eq-Edge-Sig` and, if the Worker appends rather than replaces, first-wins yields the attacker's value. Case variants, whitespace, and obs-fold add more shapes.
4. **No binding to the request.** With no method/path/body/IP in the MAC, a captured header set is replayable against a _different_ request — e.g. transplanting a good bot-score attestation onto a POST to a mutation endpoint.
5. **Path canonicalization across the hop.** The Worker sees `/api/x`; Traefik strips the prefix so the origin sees `/x` (`docker-compose.yml:191`). Whichever form is signed, both sides must agree; the design is silent.
6. **Replay window.** ±30s is _both_ too tight for operations (Worker→CF→Traefik latency, retries, clock skew) _and_ a 60-second replay window. Note the repo already ships a 5-minute webhook skew contract (`MAX_SKEW_SECS = 300`, `rux-webhook-crypto/src/lib.rs:6,36-38`); a second, stricter, undocumented clock contract is a defect in itself.
7. **Key distribution.** A shared HMAC key must live in two places (Cloudflare Worker secret + API env), with two rotation paths, and the origin then holds a key that can _forge_ edge claims. The repo already has Ed25519 verification (`verify_ed25519`, `rux-webhook-crypto/src/lib.rs:101-110`).
8. **Fail-open vs fail-closed is unspecified, and absence is indistinguishable.** The origin cannot tell "Worker did not run" from "headers were stripped". Fail-closed on absence takes the whole site down on any Worker fault or un-routed path; fail-open silently drops the control.
9. **No kid convention exists.** The repo's only `key_id` values are provider credentials (Razorpay, Apple). A kid format, a key list, and an unknown-kid policy must be defined.
10. **Scope creep.** There is **no Worker in this repo** — `X-Eq-Edge` has zero hits, no `wrangler` config, no worker source. This is a new deployment surface (CF Worker + secrets), not an extension of existing code.

### 4.2 The thing the design missed: the origin already knows it came through Cloudflare

In production, `IP_SOURCE=CfConnectingIp` is mandatory (`settings.rs:208-215`), `CF-Connecting-IP` is 400'd if absent, and the header is peer-gated to `TRUSTED_PROXY_CIDRS` (`client_ip.rs:64-74,113-121`). Origin ingress is meant to be locked to Cloudflare ranges (`deploy/README.md:377-400`), but that is an unverified owner step and the default trusted set is all of RFC1918, so this distinction holds only once ingress trust is fixed (F1-ingress-trust). Cloudflare also always sets `cf-ray`. Therefore:

- "not through CF" vs "through CF" is already distinguishable (`cf-ray` presence, `CF-Connecting-IP` presence).
- "through CF but unsigned" vs "Worker failed" is **not** distinguishable cryptographically — only by rate/metric anomaly (a liveness heartbeat endpoint set by the Worker is the cheapest discriminator).

So the edge signature's _only_ new value is attesting `request.cf` fields the origin cannot verify itself (ASN, bot score, JA4 when available). Scope it to exactly that. Do **not** fan out into a multi-header `X-Eq-Edge-*` family — every additional header is a smuggling surface.

### 4.3 Corrected construction

```
Headers (exactly one each, all REQUIRED, all lowercase names):
  x-eq-edge-attest: <base64url(canonical_json)>   # the claims being attested
  x-eq-edge-kid:    v1:<8-hex>                    # which signing key
  x-eq-edge-ts:     <unix seconds>                # integer, no fractional part
  x-eq-edge-sig:    <base64url(ed25519_sig)>      # detached signature

canonical_json = {"asn":..,"asOrganization":..,"botScore":..,"ja4":..,"colo":..}
                  keys sorted, no whitespace, absent field => explicit null
sig_input = b"eq-edge-v1\n" || method || "\n" || scheme || "\n" || host || "\n"
         || canonical_path || "\n" || canonical_query || "\n"
         || hex(sha256(body)) || "\n" || ts || "\n"
         || cf_ray || "\n" || cf_connecting_ip || "\n" || canonical_json
```

Rules, all mandatory:

1. **Worker signs; origin verifies with Ed25519 public keys** (`verify_ed25519` already available). The origin holds no signing key → it cannot forge edge claims, and rotation is a public-key list update keyed by `kid`.
2. **Worker uses `set`, never `append`.** Origin: read with `get_all` and require **exactly one** value per name; reject duplicates, comma-joined values, obs-fold, and any header whose name is not in the closed list. Strip all `X-Eq-Edge-*` from the inbound request at the outermost layer, before any other middleware, and log when stripping actually removed something (that log line is the smuggling detector).
3. **Every listed name must be present**; a missing one invalidates the whole attestation (never "the fields we happen to see").
4. **Bind method, scheme, host, canonical path, canonical query, body digest, `cf-ray`, `cf-connecting-ip`.** Decide the path question explicitly: sign the **origin-visible** path (post-strip) and have the Worker compute it by stripping `/api` itself, or have Traefik forward `X-Forwarded-Prefix` and the origin reconstruct. Pin it in a test.
5. **Clock: accept ±120s** (a separate constant, `EDGE_MAX_SKEW_SECS`, documented next to `MAX_SKEW_SECS`). Do not add a per-request `cf-ray` single-use store: it is a hot-path state mutation behind a global lock, outside the ≤10 µs / 0-fsync request budget (A7 §1); the request binding in rule 4 already limits what a replay can do.
6. **Unknown `kid` → drop the attestation, keep the request.** Alert.
7. **Sig mismatch → drop the attestation, keep the request.** Alert with the kid. Never 401/403 the request.
8. **Enforcement is a separate, explicit boot flag** (`EDGE_ATTEST_REQUIRED`, default false, mirroring the repo's default-off escalation discipline at `settings.rs:306-379`). When true, missing/invalid attestation returns 503 **only** for the specific endpoints the operator names, and never as the sole gate for a destructive decision — pair with Turnstile/risk.
9. **Log an absence-rate metric** per route so a Worker outage is visible in minutes, not at the next incident review.

---

## 5. Evidence integrity

### 5.1 Is a hash chain worth it here? — **Not at v1. It is theater without an external anchor, and worse than theater if keyed.**

- The only writer is the API. A hash chain detects after-the-fact row edits **only** if there is an anchor the writer cannot rewrite. Without one, an attacker with DB write access rewrites the entire chain and recomputes every link. Nothing is detected.
- A **keyed** (HMAC) chain is strictly worse: a third party (auditor, regulator, court) cannot verify it without the key, and the API can still rewrite it. You pay a key, a rotation story, and a verification procedure to produce an artifact that only the suspect can validate.
- A **single global chain forbids deletion**: you cannot prune evidence rows for retention limits or erasure without breaking the chain. That collides head-on with §6.

**Correct:** v1 = append-only `audit_logs` (table exists, zero writers — `m000001_init.rs:252-264`) + deletion receipts (§6). If and when there is an external party who must be convinced, add a **per-segment** unkeyed chain (one segment per day or per case, each segment storing its own final digest) plus an **Ed25519-signed daily root** pushed out of band (email to an ops address, or a publish step). Segmenting is what makes evidence expirable; the out-of-band push is what makes tamper-evidence real.

### 5.2 What exactly gets hashed (when built)

Canonical serialization of the evidence row, excluding volatile/auto columns:

```
segment_input = prev_segment_digest
             || action || resource_type || resource_id
             || canonical_json(metadata)        # sorted keys, no whitespace, no floats
             || ts_utc_iso8601                  # fixed format, UTC, seconds precision
```

Never hash raw client-supplied bytes; canonicalize/validate first (the repo's `canonical()` discipline for `BanUnit` is the model, `dto.rs:40-42`). Never hash the autoincrement `id` or a DB-local timestamp string format. Version the segment format (`seg:v1:` prefix).

### 5.3 The sha ban — verified, and a new module does **not** trip it

The ban is scoped to Quran data paths (root `AGENTS.MD`; `rust/backend/api/AGENTS.md`; `documentation/quran-system.md`). The guard files that must be checked before adding any hashing module:

| Guard                                                                                    | What it actually checks                                                                                                                                                                                                            |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust/backend/api/tests/quran_v1.rs:1237-1275`                                           | `scripts_endpoint_carries_no_sha256` — asserts the `/scripts` JSON rows have **no** `sha256` key                                                                                                                                   |
| `rust/backend/api/tests/quran_v1.rs:1665-1691`                                           | `sources_rows_never_carry_sha256` — same for the sources rows                                                                                                                                                                      |
| `rust/backend/api/tests/quran_v1.rs:925-951`                                             | `modules_quran_v1_has_no_sqlite_refs` — `include_str!` scan of the six `src/modules/quran_v1/*` files for DB access                                                                                                                |
| `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts:11-18`                          | published catalogues must not contain `sha256`                                                                                                                                                                                     |
| `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts:20-51`                          | explicit `SHA_FREE` path list (boot loaders, cache, wire, DTOs, OPFS/download, `deploy/fetch-quran-db.sh`, `deploy/provision-quran.sh`, `scripts/quran/upload.ts`) must not match `sha256\|sha2\|createHash\|crypto.subtle.digest` |
| `rust/backend/api/scripts/quran_digest_audit.rs:1-3` + `rust/backend/api/justfile:31-32` | the golden-digest audit is `--features seed-system`, **manual only** (`just quran-audit`)                                                                                                                                          |

There is **no blanket** sha guard over `src/` or over the whole web tree (verified: no such test). A new module at `rust/backend/api/src/services/evidence.rs` (or `src/utils/evidence_hash.rs`) using `sha2::Sha256` trips **nothing**. Rules to keep it that way:

- Never place evidence hashing under `src/quran/**`, `src/modules/quran_v1/**`, `web/src/lib/quran/**`, `web/src/lib/workers/**`, `web/src/lib/offline/**`, `web/src/lib/data/translations*`, `web/src/lib/quran/catalogue.ts`, or the `deploy/*`/`scripts/quran/*` paths in `SHA_FREE`.
- Never add the new module to `SHA_FREE`.
- Never wire a hash into boot, ETag, `/scripts`, `/health`, or a cache key (the stated ban).
- Keep it visibly separate: crypto sha for auth/integrity is explicitly fine (root `AGENTS.MD`), and R2 §9.3 already carves this out.

`web/src/lib/server/__tests__/server-headers.test.ts:26` already uses `createHash("sha256")` for CSP script hashes without tripping any guard — evidence hashing is the same class of use.

---

## 6. Deletion / erasure

### 6.1 What is trivial, and what is not

Trivial: `DELETE FROM device_records WHERE id = ?` (cascades `device_sessions`). Because `token_hash`/`fp_digest` are keyed digests, the row is the only linkage; once it is gone and the fp pepper has rotated past it, residual digests in logs are not re-identifiable (R2 §8.5).

Not trivial, and the design must address:

1. **Egress.** If a device digest ever left the system (the export feed, §2.3), deletion cannot recall it. Fix the export scope first.
2. **Evidence.** Evidence rows are the point of the feature; they must survive user deletion.
3. **Proof.** A user cannot verify non-retention from outside; the design needs an artifact.

### 6.2 What the design needs — a receipt, and _not_ a tombstone table

1. **Deletion receipt** (required). On any device-record deletion, write an `audit_logs` row: `action='device.delete'`, `resource_type='device_record'`, `resource_id=<opaque case ref>`, `metadata = {"scope": "...", "deletedAt": "...", "retained": [{"what":"ban_unit","basis":"Art 17(3)(e)","until":"..."}]}`. No raw signals, no user-identifying digest. This is what the DSAR response and the user can be shown.
2. **Tombstone** (not a table). Do **not** copy the deleted digest into a tombstone table — that duplicates retention and creates a second thing to expire. The **ban record is the tombstone**: bans already live in the ban store keyed by `BanUnit` (`admin_bans_v1/dto.rs:16-43`), with the design's 12-month retention. If a device record is deleted while its ban is active, the ban unit digest persists under the ban's own legal basis, and the receipt says so.
3. **Retention boundary.** `hash_version` + `key_id` on every digest (R2 §8.1's version stamp) so old digests are self-describing and can be honestly excluded from privacy claims once their pepper ages out. Delete `DEVICE_KEY_PEPPER_PREV` at the retention boundary — a retained previous pepper is itself a re-identification capability (R2 §8.5).

### 6.3 Banned user whose evidence must be preserved while data is erased

Reconcile by **separating the identity store from the evidence store**, and by making evidence self-contained:

1. `audit_logs.user_id` is already `ON DELETE SET NULL` (`m000001_init.rs:262-263`) → deleting the user preserves evidence and nulls the link. Same for `device_records.user_id` in the proposed schema.
2. **Write the anchor into the evidence row at creation time**: store the case id **and** the device digest _as of that moment_ in the evidence metadata. Then deleting the device record later cannot orphan the evidence — the evidence no longer needs the row to exist.
3. Deleting the device record while the ban is active: keep the ban unit (the ban _is_ the legal retention), delete the record and the user link, and state it in the receipt with a basis and an expiry.
4. If a chain is ever added, it must be **segment-scoped** so an evidence segment can expire without breaking the rest (§5.1).
5. Do not over-claim. The honest statement is: "raw signals are never stored; the keyed digest is retained only while a ban is active, for a documented period, and is not re-identifiable after the pepper ages out" — and the receipt must match the code.

---

## 7. Failure modes

| #   | Failure                                                               | Current/implied behavior                                                                                                    | Correct behavior                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| --- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `DEVICE_KEY_PEPPER` unset at boot                                     | design silent                                                                                                               | **Fail closed in prod**: boot error, mirroring `state.rs:122-129` (panic) + placeholder blocklist (`state.rs:74-78`) + dev default that prod refuses (`state.rs:149-155`). Never silently disable device tracking — a silent disable makes every device "new" and quietly guts ban enforcement.                                                                                                                                                                                                                 |
| 2   | Pepper changed mid-process                                            | keys are read once into `OnceLock`; `field_crypto` explicitly rejects a conflicting re-init (`field_crypto.rs:29-37,54-62`) | Keep process-local immutable keys; a rotation is a **restart**, documented. Fail closed on mismatch (do not half-apply).                                                                                                                                                                                                                                                                                                                                                                                        |
| 3   | Rotation mid-flight (new writes under new pepper, old rows under old) | design implies uniform rotation                                                                                             | `fp` subkey: dual-key read + `key_id` + lazy rewrite. `token` subkey: **no in-place rotation** — it is a global re-enrollment event (§1.4). Without `key_id`, a mixed state is invisible: lookups miss → duplicate device records → false "new device" (step-up storm) or false bans.                                                                                                                                                                                                                           |
| 4   | Edge HMAC/signature mismatch                                          | unspecified                                                                                                                 | Drop the attested claims, continue the request as **unattested**, log + alert with kid. Never reject the request. `EDGE_ATTEST_REQUIRED=true` is the only path to fail-closed, and only for named endpoints.                                                                                                                                                                                                                                                                                                    |
| 5   | Clock skew beyond the replay window                                   | ±30s is too tight                                                                                                           | ±120s with the request binding of §4.3 rule 4; alert on skew rate; NTP required on the host. Do not silently widen the window per-request.                                                                                                                                                                                                                                                                                                                                                                      |
| 6   | Restore from backup with an old key                                   | rows written under a pepper the process no longer holds                                                                     | Keep `_PREV` for at least the retention window; store `key_id` per row so an "unknown key_id" count is a diagnosable metric; document that a restore older than the pepper retention requires global device re-enrollment (token) / re-hash (fp).                                                                                                                                                                                                                                                               |
| 7   | Multi-instance with different keys                                    | **not** a risk today: exactly one `api` container (`docker-compose.yml:113-193`), pool = 1 (`sea_connect.rs:35-36`)         | Any key change during a rolling replacement is the real window where two processes with different keys are briefly both live. Deploy key changes as add-new → restart-all → remove-old (the `_PREV` slot exists for exactly this), never a single-step swap. If the API ever scales horizontally, all of this (plus `revoked_sessions` HashSet at `main.rs:178-179`, the rate-limit L1 store, and per-process CSRF keys) needs a shared secret store — treat it as an architecture change, not a config toggle. |
| 8   | Unknown `kid` on edge attestation                                     | unspecified                                                                                                                 | Reject the attestation, keep the request, alert. Never fail the request.                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 9   | Worker unavailable / no headers                                       | unspecified                                                                                                                 | Observe-mode: fine. Enforcement-mode: 503 only for named endpoints, with a documented fallback path; alert on absence rate.                                                                                                                                                                                                                                                                                                                                                                                     |
| 10  | RNG failure minting a token                                           | must not degrade                                                                                                            | Fail the request (500). Never fall back to a weak/zero token — mirror the nonce comment at `field_crypto.rs:112-114`.                                                                                                                                                                                                                                                                                                                                                                                           |
| 11  | Evidence anchor unavailable (email/publish)                           | unspecified                                                                                                                 | Never block evidence writes; mark the root pending, retry, and record a **gap** in the segment so the chain cannot be silently forged.                                                                                                                                                                                                                                                                                                                                                                          |
| 12  | `BAN_EXPORT_TOKEN`/internal token rotation mid-flight                 | separate existing secrets; export token read once at boot (`settings.rs:498`)                                               | Unchanged; note the device feature must not add a _third_ bearer token to the export path.                                                                                                                                                                                                                                                                                                                                                                                                                      |

---

## 8. Minimum crypto surface for v1

**Build:**

1. `DEVICE_KEY_PEPPER` — one new 32-byte secret, prod boot gate + placeholder blocklist + dev default, HKDF subkeys `ruxlog-device-token-v1` / `ruxlog-device-fp-v1` in `Zeroizing` `OnceLock`s (§1.2). `DEVICE_KEY_PEPPER_PREV` only for the fp subkey.
2. Device token: 128-bit CSPRNG, **random-only**, no client entropy, stored as `HMAC(token_subkey, "tok:v1:" || token)` with `token_key_id`; revocable; one storage location; never accepted as proof of possession (§3).
3. `fp_digest = HMAC(fp_subkey, "fp:v1:" || canonical)` with `fp_version`/`fp_key_id`; canonicalization of components specified before any code (§1.2, §2.4).
4. Evidence: append-only `audit_logs` + `device.delete` receipts. **No chain, no key** (§5, §6).

**Defer:**

- Edge attestation entirely. There is no Worker, no wrangler config, and `X-Eq-Edge` appears nowhere in the repo; the origin already gets `CF-Connecting-IP` + `cf-ray` (trustworthy once the ingress path is actually CF-only — F1-ingress-trust). When it lands: Ed25519, a single canonical-JSON attestation header, the construction in §4.3.
- Ed25519-signed daily evidence roots + out-of-band anchor (build when an external party must be convinced).
- `BanUnit::Device` + the export-feed device axis (needs the IP-only enum changed and creates unrecallable digest egress). If ever built: never export the digest.

**Drop entirely:**

- Weekly epoch rotation + mapping table (§2).
- HMAC-signed device tokens (§3.4).
- Client-contributed token entropy (§3.3).
- Keyed evidence hash chain (§5.1).
- Multi-header `X-Eq-Edge-*` fan-out (§4.2).

---

## Findings

1. **NEW — Device pepper must be a new secret, not HKDF-from-`COOKIE_KEY`** · High
   _Flaw:_ the design never states the pepper's provenance; following the CSRF precedent (`static_csrf.rs:45`) would derive it from `COOKIE_KEY`, so one leak grants session forgery **plus** device-record forgery/ban evasion, and device-pepper rotation would force a session-invalidating `COOKIE_KEY` rotation. Repo convention already separates signing (`COOKIE_KEY`), encryption (`FIELD_ENC_KEY`), and service auth (`INTERNAL_QURAN_API_TOKEN`) — `state.rs:108-179`, `settings.rs:456-504`, `.env.example:3-5`.
   _Corrected:_ one new `DEVICE_KEY_PEPPER` (32 B, prod panic + placeholder blocklist + refused dev default, `state.rs:74-78,122-129,149-155`) with two HKDF subkeys (`ruxlog-device-token-v1`, `ruxlog-device-fp-v1`) using the `static_csrf.rs:17-30` helper, stored in `Zeroizing` `OnceLock`s (`field_crypto.rs:13-15`) — not `OnceLock<Vec<u8>>` (`static_csrf.rs:15`).

2. **NEW — Token subkey is non-rotatable; design must say so** · Medium
   _Flaw:_ only `HMAC(pepper, token)` is stored, so a pepper rotation cannot re-hash existing rows — every device record is permanently orphaned and every `BanUnit::Device` stops matching. The design implies all keys rotate uniformly.
   _Corrected:_ `token_key_id` column; current + prev token peppers; document rotation as a deliberate global re-enrollment/invalidation event (post-breach). fp subkey rotates freely via dual-key read + lazy rewrite.

3. **NEW — Weekly epoch rotation + mapping table: drop it** · High
   _Flaw:_ the mapping table's stable pseudonym is exactly the join rotation exists to break (zero unlinkability gain), while dropping the table means a banned device evades by waiting out the epoch. The only viewer of the digest is the process holding the key. Cost: ~52 rows/device/year, an extra lookup on a **pool=1** SQLite DB (`sea_connect.rs:35-36`), a rotation job on the process-local scheduler (`main.rs:761`), and a second overlapping dual-key window.
   _Corrected:_ stable `device_records.id` (ULID) + versioned, dual-key-rotatable `fp_digest`. Schema in §2.4.

4. **NEW — Ban export feed must stay IP-only** · High
   _Flaw:_ `BanUnit::Device(device_key)` would serialize the pepper digest into the export feed consumed by a proxy adapter outside the repo (`dto.rs:40-51,90-100`; `deploy/README.md:419-429`) — unrecallable egress, broken erasure claims, epoch re-export churn, and it breaks the deliberate "unparseable keys can never pose as exportable ban units" invariant (`dto.rs:13-15`).
   _Corrected:_ export stays IP-only; any device axis for the adapter uses a per-deployment opaque reference with an explicit expiry, never the digest.

5. **NEW — Device token is an identifier, never an auth factor** · High
   _Flaw:_ the design puts a server-minted token in localStorage+IDB (`DESIGN-v0.md:34`) without stating what possession proves. If it can skip step-up, reading localStorage = instant device-record impersonation and an ATO enabler; NIST 63B-4 §5.1 (R2 §5.3) says session secrets SHOULD NOT live in localStorage.
   _Corrected:_ token answers "which record", never "same machine"/"legitimate user"; all trust from server-side signals; ≥128-bit CSPRNG; stored as HMAC; `revoked_at` + revoke UI + per-user cap; one storage location; optional bind to the existing `session_auth_hash` (`rux-auth/src/session/extractor.rs:19-27,53`) as a signal only.

6. **NEW — Token: random-only, server-minted, no signature, no client entropy** · Low
   _Flaw:_ the design leaves signed-vs-random and client-entropy open. A signature adds a key/kid/expiry/rotation for zero gain when the only verifier is the process with the DB; client entropy leaks together with the token under XSS/malware and orphans devices on site-data clear.
   _Corrected:_ random-only opaque token; if IDB is used it needs a **new** DB name because `IDB_VERSION` is pinned at 1 with a one-store-per-DB invariant (`web/src/lib/workers/idb.ts:2,19-31`); prefer the existing localStorage helpers (`web/src/lib/storage/safe-storage.ts`).

7. **NEW — Edge canonical string unspecified → canonicalization/smuggling surface** · Critical
   _Flaw:_ no canonical string defined; a MAC over "the `X-Eq-Edge-*` headers present" signs an attacker-influenced set; `HeaderMap::get` is first-value-wins (`rux-webhook-crypto/src/lib.rs:29-34`); Traefik forwards unknown headers and its only api middleware is `stripprefix /api` (`docker-compose.yml:189-192`), so client-supplied `X-Eq-Edge-*` reaches the origin.
   _Corrected:_ fixed, closed, ordered header list; all names REQUIRED; origin reads with `get_all` and requires exactly one value; strip client `X-Eq-Edge-*` outermost and log when stripping removed something; length-prefixed/canonical-JSON signed input; Worker uses `set`, never `append`.

8. **NEW — Edge signature is not bound to the request; ±30s is both too tight and replayable** · High
   _Flaw:_ no method/path/query/body/IP binding → a captured attestation replays against a different request; 30s fights Worker→CF→Traefik latency and retries while still allowing 60s replay; the repo's existing skew contract is 300s (`rux-webhook-crypto/src/lib.rs:6,36-38`), so a second undocumented clock contract is itself a defect.
   _Corrected:_ bind method, scheme, host, canonical path, canonical query, `sha256(body)`, `cf-ray`, `cf-connecting-ip`; accept ±120s (`EDGE_MAX_SKEW_SECS`); no per-request `cf-ray` dedupe store (hot-path cost, A7 §1).

9. **NEW — Path canonicalization across Traefik `stripprefix`** · Medium
   _Flaw:_ Worker signs `/api/x`, origin sees `/x` (`docker-compose.yml:191`); whichever form is signed, both sides must agree — the design is silent.
   _Corrected:_ pin one canonical form (recommend the origin-visible path, Worker strips `/api` itself), or forward `X-Forwarded-Prefix` and reconstruct; add a test that pins it.

10. **NEW — Use Ed25519 for edge attestation, not a shared HMAC key** · High
    _Flaw:_ a shared HMAC key lives in two stores (Worker secret + API env), needs two rotation paths, and gives the origin the ability to forge edge claims.
    _Corrected:_ Ed25519 — private key only in the Worker, public keys + `kid` list at the origin; `verify_ed25519` already exists (`rux-webhook-crypto/src/lib.rs:101-110`); rotation is a public-key swap; unknown `kid` → drop attestation, keep request.

11. **NEW — Edge fail-open/closed unspecified; absence indistinguishable** · Critical
    _Flaw:_ the origin cannot tell "Worker did not run" from "headers stripped". Fail-closed on absence takes the site down on any Worker fault; fail-open silently drops the control. The design also has no Worker at all in-repo (`X-Eq-Edge` zero hits; no wrangler config).
    _Corrected:_ observe-mode default; mismatch/unknown-kid/missing → drop claims, keep request, log + alert; enforcement behind an explicit default-off `EDGE_ATTEST_REQUIRED` flag (mirroring the default-off escalation discipline at `settings.rs:306-379`), only for named endpoints, never the sole gate; alert on absence rate; use `cf-ray`/`CF-Connecting-IP` (already mandatory in prod, `settings.rs:208-215`) to distinguish "not through CF" from "through CF, unsigned"; scope attestation to `request.cf` fields the origin cannot verify.

12. **NEW — Evidence hash chain: drop for v1; segment + external anchor later** · Medium
    _Flaw:_ without an anchor the DB writer rewrites the whole chain; a keyed chain cannot be verified by any third party and is still rewritable; a single global chain forbids retention/erasure deletes.
    _Corrected:_ v1 = append-only `audit_logs` (`m000001_init.rs:252-264`) + receipts; later, per-segment unkeyed chain over `(prev_segment_digest, action, resource_type, resource_id, canonical_json(metadata), ts_utc_iso8601)` with an Ed25519-signed daily root pushed out of band. Sha guards verified — a new module outside the Quran paths trips none: `tests/quran_v1.rs:1237,1665,925-951`, `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts:11-51`, `rust/backend/api/scripts/quran_digest_audit.rs` + `rust/backend/api/justfile:31-32` (manual, seed-system). Never place evidence hashing under `src/quran/**`, `src/modules/quran_v1/**`, `web/src/lib/quran|workers|offline/**`, or the `SHA_FREE` list.

13. **NEW — Erasure vs ban evidence: receipts, self-contained evidence, no tombstone table** · High
    _Flaw:_ design is silent; a user cannot prove non-retention; deleting the device row orphans evidence that points at it; a tombstone table duplicates retention.
    _Corrected:_ `device.delete` receipt row in `audit_logs` (opaque case ref, scope, `retained[]` with basis + until, no digest); write the case id **and** the digest into the evidence row at creation so it survives row deletion; `audit_logs.user_id`/`device_records.user_id` NULL-on-delete (`m000001_init.rs:262-263`); the ban store (not a tombstone) is the retention claim with documented 12-month expiry; delete `DEVICE_KEY_PEPPER_PREV` at the boundary; segment any future chain.

14. **NEW — Failure-mode table missing; single-instance/process-local assumption unstated** · High
    _Flaw:_ the design does not state behavior for unset pepper at boot, mid-flight rotation, restore-from-backup with an old key, or mixed keys across processes. Today the stack is exactly one `api` container (`docker-compose.yml:113-193`) with pool=1 (`sea_connect.rs:35-36`) and several process-local assumptions already load-bearing (`revoked_sessions` `main.rs:178-179`; per-process CSRF key `static_csrf.rs:15`), so a rolling image replacement is the realistic window where two processes with different keys are briefly live.
    _Corrected:_ prod boot error for an unset/placeholder pepper; keys immutable per process, rotation = restart (`field_crypto.rs:29-37` precedent); `key_id` on every digest so mixed/unknown-key state is a metric, not silence; keep `_PREV` for at least the retention window; deploy key changes add-new → restart-all → remove-old; treat horizontal scaling as an architecture change requiring a shared secret store. Full table in §7.
