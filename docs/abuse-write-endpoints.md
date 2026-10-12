# Write-endpoint abuse prevention — implementation slice

Status: **plan, not implemented.** Baseline `38f12b44` on `next-features` (2026-10-12).

Sequenced by: phase **A1** in [`master-plan.md`](master-plan.md).

Relations: [`abuse-prevention.md`](abuse-prevention.md) (master) is the authority on abuse,
enforcement, device identity, and legal artifacts — this document **amends it, never competes**.
It exists because that audit was measured at `53d3d421`, before the `next-features` branch landed
`m000012`–`m000014` and three new authenticated write surfaces. Where the two overlap, the
master plan's vocabulary and acceptance style are reused verbatim so the workstreams merge
without renaming.

---

## 0. What this branch already landed (delta vs the master audit)

Measured at `38f12b44`. The master audit's W13 items already satisfied here:

| W13 item                   | Status on this branch                                                      | Evidence                                           |
| -------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------- |
| Per-domain quotas          | ✅ 50 sessions, 10 000 live bookmarks, new-id gating, skipped surfaced     | `reading_session.rs:18,116`, `bookmark/actions.rs` |
| Clamp client `updated_at`  | ✅ stricter than W13 asks: `min(updatedAt, now + 60s)` (W13 proposes 24 h) | `reading_session_v1/controller.rs:23-32`           |
| Verse-reference validation | ✅ per-surah ayah counts (not the flat 1–286 W13 found)                    | `bookmark_v1/controller.rs ensure_valid_range`     |
| Range span cap             | ✅ 50, both endpoints checked                                              | same                                               |
| Ownership guards           | ✅ `expectedUserId` + live-session ownership (EC-52)                       | `reading_session.rs:238-251`                       |

**Not satisfied — the gap this slice closes** (all three apply to surfaces that did not exist
at the master audit's baseline):

| Gap                                                   | Exposure today                                                                                                                                                                      |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No per-account limiter on `/reading-session/v1/sync`  | 200 mutations × 100 IP-rounds/min per IP; a user on rotating IPs gets N× that — the exact same-user-multi-IP case. Server work per round is LWW + tombstone + re-home transactions. |
| No per-account limiter on `/user/analytics/v1/events` | batch ≤ 100 × 100 rounds/min/IP; every batch writes rows + runs the 90-day purge. Cheapest amplification surface in the app.                                                        |
| No folder cap                                         | W13 asks 500 folders/account; `m000012` added `kind`/`position`/tombstones but the folder upsert has no per-user ceiling (bookmarks do).                                            |
| No snapshot-size guard                                | both sync endpoints return the full per-user snapshot; a capped account that keeps pushing gets a full replay every round.                                                          |
| No cross-domain daily write ceiling                   | one user can stay under every per-route budget while writing ~10⁶ rows/day across bookmarks+sessions+analytics combined.                                                            |

## 1. Design — subject-keyed limits (the "same user, different IPs" answer)

The master plan's key-class vocabulary (`bookmark:sync:user:{id}`) is extended to a uniform
**subject limiter** for every authenticated write route:

```
limit subject = (verified user id, route class)
dual bucket   = min(ip+route bucket, user+route bucket)   // both must pass
anon fallback = existing ip+route bucket unchanged
```

- **Key classes added**: `session:sync:user:{id}` 60/60 s; `analytics:events:user:{id}`
  30/60 s; `bookmark:sync:user:{id}` 60/60 s (W13's number, now real). No long tier on any of
  them — a 429 + `Retry-After` ≤ 15 min, matching W9's shape. Unauthenticated requests keep the
  IP-only layer (the account view does not exist yet).
- **Why user-keyed is correct here**: the write surfaces are all `auth_guard::verified`; the
  user id is server-derived (cookie), not client-authored, so rotating IPs cannot mint new
  subjects. NAT/CGNAT false positives disappear because the binding unit is the account.
- **Daily ceiling**: one in-memory TTL counter per user across the three route classes —
  `writes:user:{id}` at 5 000 accepted mutations/day (E; tune after W3 measurement lands).
  Exceeding it returns 429 with `Retry-After` to midnight UTC and writes one
  `rate_limit_events` row (W3's table) on the transition only, never per request.
- **Response envelope**: both sync endpoints already return `{applied, skipped}` — add
  `rejected` and `capped` counters (W13's contract) so a rate-limited batch is visible to the
  client store instead of indistinguishable from a soft skip.

**Deliberately not built** (master plan §1 non-goals bind): no browser fingerprint key, no
device-keyed bucket, no ASN heuristics, no behavioural scoring. The subject is the account.

## 2. Fingerprint seam (how the device-identity workstream plugs in — no duplicate concept)

The master plan §6 device-identity work becomes a **signal producer**, never a key owner:

```
rate-limit decision  = f(subject limits)                    // this slice, ships first
anomaly detection    = g(device-key coherence, W6 audit)    // device-identity workstream
step-up response     = Turnstile challenge (already integrated)
```

- The limiter takes an optional `subject_score` parameter (default 1.0). The device-identity
  workstream later supplies it from server-side coherence signals; a low score **never blocks** —
  it lowers the per-user budget floor (e.g. ×0.5) and raises the daily-ceiling event to the
  operator queue. One seam, one consumer, zero key conflict.
- Client-supplied identifiers (fingerprints, device ids) are scored inputs under master §3's
  adopted/dropped table — they already rejected component fingerprints as ban keys (uniqueness
  18.5–45.52% churn measured); nothing in this slice contradicts that.

## 3. Edge layer (proxy)

Traefik (Dokploy) routes but does not rate-limit. The decision is **out-of-repo**: put
Cloudflare (or equivalent) in front for volumetric L3/L4 + a coarse WAF rule on `/api/*`
(untouched by this slice — app-level rules below assume it may not exist). The app's per-IP
layer stays the authoritative backstop; ingress trust (forged `cf-connecting-ip`) is already
W0's contract and gates every per-IP key this slice keeps.

## 4. Build order and acceptance

| Step | Work                                                                                                                 | Acceptance                                                                                                                                                                                             |
| ---- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| S1   | Folder cap 500/account (`new ids only`, like the bookmark quota)                                                     | amplification test cannot exceed 500+cap-rejected counters                                                                                                                                             |
| S2   | Subject limiters: key classes + dual-bucket min in `rate_limit.rs`; `rate_limit_layer` gains a verified-user variant | a user at 2 IPs cannot exceed the user bucket; a 429'd user's next request carries `Retry-After`; `tests/rate_limit_user.rs` proves bucket keys contain the user id, never the IP, for subject classes |
| S3   | Daily cross-domain ceiling + one transition event                                                                    | 5 001st mutation 429s; exactly one `rate_limit_events` row per day per user                                                                                                                            |
| S4   | `rejected`/`capped` envelope + snapshot 507 guard over cap                                                           | capped account gets 507, never a silent truncation; client store surfaces it                                                                                                                           |
| S5   | `subject_score` seam parameter (default 1.0, no consumer)                                                            | typechecks; documented here; device-identity workstream is the only future writer                                                                                                                      |

All steps land in `rust/backend/api` + migration only (S4 touches the web client store's error
surface). Tests reuse the session-cookie harness from `tests/bookmarks_v1.rs`. Estimated
S1–S2 one day, S3–S4 one day, S5 minutes.

## 5. What this document does not decide

Account bans, enforcement notices, legal artifacts, device-key storage — all owned by
`abuse-prevention.md` W5–W8 and §6. If that plan's phases renumber, only §4's references need
updating.
