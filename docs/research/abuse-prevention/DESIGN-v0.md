# Design sketch v0 — device identity for EasyQuran

Goal: secure authentication + make banning of abusive/illegal actors effective.
NOT ad tracking. NOT behavioral biometrics. NOT a bot-vendor buy.

Repo facts that constrain everything (verified):

- Rust API: axum, SQLite only, pool = 1 connection, migrations auto-run at boot (m000001..m000011).
- Session: tower-sessions opaque id in cookie `ruxlog.sid` (HttpOnly, SameSite=Lax, `Secure` per `cookie_secure`, `Path=/`, 14 d inactivity expiry), msgpack in `sessions` table, payload `rux_auth` = AuthSessionState. `user_sessions` audit row per session, `auth_session_binding` links tower id <-> audit row. Revocation = process-local HashSet + DB `revoked_at`.
- Guards: `auth_guard` (authenticated/unverified/verified/role), ban check with 5-min cache, fail-closed.
- IP: single trusted signal `CF-Connecting-IP` (IP_SOURCE hardcoded), peer-gated by TRUSTED_PROXY_CIDRS; 400 if absent in prod.
- Rate limit: `rux-request-gate`, fixed-window in-memory L1 + SQLite L2 flush every 10s; keys `ratelimit:{unit}:{path}` where unit = IP /32 or /64, `internal-webssr`, `unknown`. Escalation default-OFF, IP-only, allowlist-gated, never permanent.
- Web: SvelteKit 3 + Svelte 5, one session/account API choke point `AuthClient.#request` (web/src/lib/auth/auth-client.ts:167-229). `/t/**` and `/{surah}/t/**` are SSR + 7-day disk cache and are bypassed whenever a cookie is present. CSP: script-src 'self' + nonce + wasm-unsafe-eval; connect-src includes challenges.cloudflare.com.
- Test: web = vitest+happy-dom (no canvas/WebGL/audio/UA-CH/IDB in happy-dom); rust = in-memory sqlite + Migrator::up, `--test-threads=1`.
- Edge: Cloudflare -> Traefik (TLS terminator) -> web:8080 / api:8888. App never sees a TLS ClientHello. JA4 only on CF Enterprise+Bot Management. Logpush available all plans. No CF bot-score header reaches origin today.

## Layer 0 — Collection policy (what we refuse to collect)

- No canvas/audio/font-enumeration hashes as identifiers (noised by Safari 26 AFP for all browsing, Firefox RFP, Brave farbling; revertible randomization is worse than none). Canvas/audio at most as fuzzy cohort inputs, never keys.
- No behavioral biometrics (GDPR Art 9 biometric-for-unique-ID; 80% F1 in lab; false positives).
- No raw UA/UA-CH storage, no raw screen values, no precise geolocation, no `navigator.plugins`.
- No blocking/penalizing privacy browsers, VMs, devtools, or VPN use.

## Layer 1 — Client collector (`web/src/lib/device/`)

- Dynamically imported after first paint, `browser`-gated, storage in try/catch, follows safe-storage conventions.
- Emits a compact payload:
  - hardware class: WebGL/WebGPU vendor+renderer family + extension set hash + shader precision; WebAssembly/JS timing signature
  - platform class: `platform`, JS feature matrix, UA-CH high-entropy values when available
  - locale class: language quintuple + Intl resolver
  - screen class: coarse bucket only
  - coherence booleans: UA<->UA-CH<->feature matrix, GPU<->platform, hardware triple, screen vs inner
  - spoof/tamper flags: `navigator.webdriver`, native-fn tampering, CDP artifacts
- A first-party device token: server-minted opaque id, stored in localStorage + IDB (new DB name; IDB_VERSION is pinned at 1).
- Transport: attached to `AuthClient.#request` only (all `/api/**`), never on `/t/**` reader routes, so the 7-day disk cache is untouched.

## Layer 2 — Server storage (m000012)

- `device_records`: id, device_key (HMAC-SHA256 of components with server pepper), user_id NULL (SET NULL), first_seen, last_seen, seen_count, trust_state, coherence booleans, evidence JSON, created_at. UNIQUE(device_key). Index (user_id), (last_seen).
- `device_sessions`: link user_session_id <-> device_record_id.
- Extend ban scope with a device axis: `BanUnit::Device(device_key)` alongside IPv4 /32 and IPv6 /64.
- Reuse the dormant `audit_logs` table for evidence (currently has model + migration, zero writers).

## Layer 3 — Risk + enforcement

- Deterministic rule engine with versioned rules, two thresholds (ban >= 80, restore <= 40) = hysteresis.
- Ladder L0 observe -> L1 throttle -> L2 challenge (Turnstile) -> L3 cooldown -> L4 hold for review (<=72h) -> L5 suspend -> L6 ban + second look -> L7 legal referral.
- Step-up (existing 5-min fresh-auth grants) triggered on: new device at login, device change mid-session, credential change from a new device, ban-relevant action.
- Never auto-permanent-ban (Dutch AP Uber fine: automated deactivation without human intervention).
- Shared-device safety: device signal may challenge freely but may only ban with corroboration (network + credential + behavior/content).

## Layer 4 — Edge

- Cloudflare Worker adds signed `X-Eq-Edge-*` headers (request.cf fields, JA4 only when the plan exposes it), HMAC-SHA256 + timestamp + kid, +/-30s replay window; origin strips client-supplied `X-Eq-Edge-*` and verifies.

## Layer 5 — Ops

- `/admin/devices` API (list/lookup/ban/unban/evidence), minimal UI, appeal flow with durable notice, precision/overturn/TTD dashboard.

## Layer 6 — Compliance

- Purpose limitation in code: device records never joined to reading/recitation/search data; no inference of religious belief (Art 9).
- Retention ladder: raw signals never stored -> keyed device key 30d -> ban record 12mo -> aggregates forever.
- Lawful basis Art 6(1)(f) + ePrivacy strictly-necessary for auth; UK DUAA 2025 s.112 explicitly permits security/fraud/authentication fingerprinting. DPIA. DSAR path for device records.
