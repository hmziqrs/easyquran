# Device contract

What every client must do for account devices.

## Edge cases

| Case | Required behaviour |
| --- | --- |
| More than 20 devices | Oldest idle evicted, never the caller |
| Sign-out on a shared phone | Push token deleted first |
| Device removed while its threads remain | Label falls back to device kind; its dirty rows still land as sent |
| Unknown device id on a thread | Stored as sent; resolves to no name |
| Reinstalled device with a live hint | Name carries over the merge; threads under the old id still resolve through `merged_into` |

## Handoff labels (D5)

- A pulled thread row carries `deviceName`: the name of its `deviceId`, followed through `merged_into` in the user's own rows. Null for an unknown, revoked or unnamed device, and never sent back.
- A cursor move writes the cursor's `device` and `deviceId` from the install that made it. Lifecycle-only writes keep the last cursor writer.
- "Here" means the same `deviceId`. Two phones are two devices. A row with no `deviceId` falls back to comparing the kind.
- Label order: this device (no label), then the device's name, then "on your {kind}".
- Not yet built: export and import (F6) must drop `device_id` and stamps, and import may only add ids it does not hold.

## Device keys and continuity (D3)

Vectors: `contracts/device-vectors.json`. Passed by `rust/backend/api/tests/device_vectors.rs` and `web/src/lib/device/__tests__/keys.test.ts`. Any client must pass it.

### Key

- One P-256 key pair per account per install. The server stores only the public JWK, with exactly the members `kty`, `crv`, `x`, `y`. Any other member is refused.
- Thumbprint: RFC 7638 over the canonical text `{"crv":"P-256","kty":"EC","x":"<x>","y":"<y>"}` (no whitespace, that member order), SHA-256, base64url without padding. `x` and `y` are 32-byte coordinates in base64url.
- A key is the browser's non-extractable WebCrypto key on web, and Secure Enclave, Keystore or OS keystore elsewhere. The server never sees the private half.

### Signature

- Wire form: raw `r || s`, 64 bytes, base64url without padding. WebCrypto already produces it. iOS and Android produce DER and must convert:
  - `SEQUENCE { INTEGER r, INTEGER s }`, outer tag `0x30`, short or one-byte long length.
  - Each INTEGER is minimal: a leading `0x00` only when the next byte has its high bit set, and never on a value without it.
  - Left-pad `r` and `s` to 32 bytes each. Anything else is refused.
- Proof message: UTF-8, four lines joined by LF, no trailing newline:
  `easyquran-device-v1` LF nonce LF device id LF user id in decimal. The nonce is the base64url text as sent.
- Verification is ECDSA P-256 with SHA-256 over the message, against the uncompressed point `0x04 || x || y`.

### Challenge

- `POST /device/v1/challenge` returns `{ "nonce", "salt" }`.
- Nonce: 32 random bytes, base64url. One outstanding per session. Single use, five minutes. It is bound to the session that asked for it. A full map refuses new challenges and never evicts a live one. Process-local, so a restart means the client asks again.
- Salt: 32 random bytes, base64url. One per account, created on first use and never rotated. Two accounts get different salts.

### Continuity hint (Android, Windows, Linux only)

- `hint = base64url(HMAC-SHA256(key = salt bytes, message = normalised OS id))`.
- Normalised OS id: trim ASCII whitespace (space, tab, CR, LF), then ASCII lower-case.
- Sent only with a key. A hint that arrives without a key is ignored. A hint with a key must be 32 bytes once decoded.
- Apple platforms send no hint. Web sends no hint.

### Register

`client` and `platform` pair up: `web` sends `web`; `react_native` sends `ios` or `android`; `gpui` sends `macos`, `windows` or `linux`; `nsh` (Native Shell) sends any of `ios`, `android`, `macos`, `windows` or `linux`. Only `web` sends `browser`.

Fields added to `POST /device/v1/register`:

| Field | Rule |
| --- | --- |
| `key_store` | Required. Web: `browser` or `none`. React Native: `secure_hardware` or `none`. GPUI: `os_keystore`, `file` or `none`. NSH: `secure_hardware` (iOS Keychain, Android Keystore), `os_keystore`, `file` (a 0600 file) or `none` |
| `public_key` | Present exactly when `key_store` is not `none` |
| `nonce`, `signature` | Both present with a key, both absent without one |
| `continuity_hint` | Optional, only with a key |

Reply: `{ "registered": true, "device_id": "<id to keep>" }`. The id can differ from the one sent. The client keeps the id in the reply.

Refusals:

- `400` "Device key proof failed": wrong signature, a nonce that is missing, used, expired or issued to another session, or a malformed key. One message for all of them.
- `409`: the id was removed, a known id has a different key that this session does not hold, or a valid key arrives with a different hint. The client mints a new id and key and registers once more, keeping local rows and dirty data.

### Recognition

Within one account only. First match wins. Every read and write takes `user_id`.

1. Known id, key matches the stored thumbprint, hint matches or absent: same row. A stored hint is kept, and a missing one is filled in.
2. The current session holds a live row, and either the id is unknown or the key was lost: that row, new key, and the reply carries its id. Holding the session is the proof.
3. Unknown id, key, and a hint that matches a live row with no conflict: the new id gets its own row. The old row is retired with `merged_into` set to the new id, and the old thumbprint is kept on the new row as `merged_key_thumbprint`.
4. Otherwise a new row. Older rows with the same platform, client, browser and form factor and no live session are retired. A row with a live session is another device and is never merged on coarse traits.

Shared hint (rule 7): a merged row presents its old key again with the same hint, and a live row still carries that hint. Both rows get `hint_conflict`, the old row is restored, and neither machine is signed out. A flagged hint never matches again under rule 3.

Retired and merged rows never appear in the device list. Sync rows keep their device id, and labels resolve through `merged_into` (D5).

### Edge cases covered by tests (D3)

| Case | Covered by |
| --- | --- |
| Signature formats differ by platform | `device_vectors` DER and raw tests; `keys.test.ts` |
| Browser key, no key, and two tabs racing a first sign-in | `key-store.test.ts`, `register.test.ts` (keyed and keyless) |
| Private window, insecure origin, WebCrypto failure | `register.test.ts` (key store unusable registers with `key_store: none`) |
| Two browsers, or browser and installed PWA, on one machine | `rung_four_two_sessions_without_a_hint_are_two_devices` |
| Second account signs in on one device | `the_same_machine_hints_differently_under_two_accounts`, `an_account_only_ever_sees_its_own_rows_for_recognition` |
| Safari 7-day cap or site data cleared, session alive | `rung_two_session_holds_the_row_so_a_lost_key_keeps_the_row` |
| Keychain item visible but key unusable | Server: `rung_two_session_holds_the_row_so_a_lost_key_keeps_the_row`. Detection on iOS is manual |
| Two machines sharing a hint (`MachineGuid`, `machine-id`, clone) | `rule_seven_a_shared_hint_is_detected_and_neither_machine_is_signed_out` |
| Full disk clone with keys intact | `rung_one_same_key_and_hint_is_the_same_row` (same id and key on two installs: one row) |
| Remote sign-out while offline | `a_removed_id_cannot_register_again` (D2) |

Manual only (need a device): iOS and Android reinstall (server side: `rung_three_*`), Android lock-screen removal, Android device-to-device transfer, Android multi-user and work profile, backup restored onto a new phone or Mac, phone locked during background sync, and iOS key detection.
| Keychain item visible but key unusable | Treated as lost |
| Android lock screen removed | No user-auth binding, so key survives |
| Phone locked during background sync | Sync uses no key |
| Web IndexedDB or `localStorage` cleared, session alive | Rung 2: same row |
| Private window, insecure origin, WebCrypto failure | `key_store: none`; still reads and syncs |
| Two browsers, or browser and installed PWA, on one machine | Separate devices, by design |
| Two machines sharing `MachineGuid` or `machine-id` | Detected on the second install; rows split; nobody signed out |
| Full disk clone with keys intact | Accepted limit: one row; sync still correct |
| Android multi-user or work profile | Separate devices |
| Tablet, foldable, or desktop-mode iPad changes form factor | Latest register wins; label follows the last write |
| Device removed while its threads remain | Label falls back to device kind |

## Session transport (D6)

Native clients (React Native, GPUI, NSH) have no cookie jar. They keep the session value in the D3 secure store and send it in `x-eq-session`. The server copies that value into the one `Cookie` it sees, so every session, CSRF, rotation and revocation rule is the same as on web.

### Request and response

- Base URL: the API root. In production the proxy serves the API under `/api` (prefix stripped), so the base URL ends in `/api`.
- Request header `x-eq-session` is sent on every call. Empty means no session yet. Any `Cookie` header the client sends is replaced, never appended. Clients send `credentials: 'omit'` (React Native) or use no cookie store (GPUI, NSH).
- Response: the session value comes back in `x-eq-session` whenever the session cookie was set or rotated. Empty means the session was cleared. No response carries `Set-Cookie`. Keep the newest value after every response that has the header; keep the old one when the header is absent.
- A value with characters outside base64, `+ / = - _ . %`, or longer than 4096 bytes is refused with 400.
- A request with `x-eq-session` and either `Origin` or any `Sec-Fetch-*` header is refused with 403. The header is not in the CORS allow list and is not exposed to scripts.
- Never log the header value, or put it in traces or error reports.
- CSRF: fetch `csrf-token` from `POST /csrf/v1/generate` with the session. The token derives from the session id, so refetch it after any `x-eq-session-rotated: 1` and after any new session.
- Rotation: sign-in cycles the session id and sends `x-eq-session-rotated: 1`. A lost rotation response shows up as 401 on the next call. Then sign in again. Dirty sync rows wait until then.
- Sign-out: `POST /auth/v1/log_out` flushes the session. The reply has an empty `x-eq-session`, and the old value returns 401.
- Sign-in may answer with a `pending_login` body (TOTP required). Handle it on every sign-in path before treating the session as signed in. Unverified accounts see a verify step on `verified` routes.

### Lifetimes and revocation

- Web sessions: 14 days of inactivity.
- Native session registered with a verified device key (`key_store` not `none`): 180 days of inactivity. The idle timer is pushed forward at most once a day, so the real floor is 179 days.
- Revoke (device delete), bans and sign-out end a native session at once: the next call is 401.
- A native session registered without a key keeps the 14-day default.

### Sign-in matrix

NSH follows the iOS and Android columns on phones and the desktop column on macOS, Windows and Linux.

| Method | iOS | Android | Desktop (GPUI, NSH) | Check |
| --- | --- | --- | --- | --- |
| Email and password | Native form on `/auth/v1/log_in` | Same | Same | Test: `native_client_full_flow_with_no_origin_and_no_cookie_jar` |
| Email sign-up | Native form on `/auth/v1/register`, no `turnstile_token`. Native requests skip Turnstile; browsers still need it. The per-IP route budget and the creation caps (per IP and global, daily) still apply | Same | Same | Test: `native_sign_up_skips_turnstile_and_a_browser_does_not` |
| Google | Provider SDK, then `POST /auth/google/v1/token` | Same | System browser with PKCE and a `127.0.0.1` loopback redirect, then `/token` | Test: `native_audience_tests` (audience list). Desktop: manual per O32 |
| Other providers | Provider SDK, then `/auth/{provider}/v1/token` (Apple, Facebook, GitHub, Discord) | Same | Only after checking the provider's current desktop rules | Existing endpoints. Manual per O32 |
| Sign in with Apple | Required next to other social sign-ins (O31) | Optional | Optional | Manual (O31) |
| Passkey | Associated Domains on the RP (N4) | `android:apk-key-hash:<hash>` origin from `WEBAUTHN_ANDROID_APK_KEY_HASHES` (Play App Signing key) | Not at launch | Tests: `android_origin_is_the_apk_key_hash_form`, `service_builds_with_android_origin_in_dev`. Registration on device: manual |

Env: `GOOGLE_MOBILE_CLIENT_IDS` and `GOOGLE_DESKTOP_CLIENT_IDS` are comma lists next to `GOOGLE_CLIENT_ID`. Every listed id is an accepted audience.

### Push matrix

| Device | Push | Note |
| --- | --- | --- |
| iOS | FCM through APNs (APNs key, O34). Alerts carry an `apns` block: `alert` type, priority 10, default sound | Test: `push_hint_contains_no_account_or_notification_details`. Delivery: manual |
| Android | FCM | Existing |
| Web | FCM web push | Existing |
| Windows, Linux | None. No FCM client | Local notifications only |
| macOS (GPUI, NSH) | None at launch | Local notifications only |

A desktop device (GPUI, or NSH on macOS, Windows or Linux) registers without a push token. The register call accepts no `token` (covered by the device registry tests), so the client must not show push as available.

### Edge cases covered (D6)

| Case | Covered by |
| --- | --- |
| Native app unused for weeks | `native_session_outlives_thirty_idle_days_and_web_does_not`: stored expiry more than 170 days out after a keyed register and again after a later request; web session under 15 days |
| Session rotation on a native client | `native_client_full_flow_with_no_origin_and_no_cookie_jar`: `x-eq-session` changes on sign-in, rotated header set, CSRF refetched, no `Set-Cookie` |
| Browser tries to use `x-eq-session` | `a_browser_cannot_use_the_native_session_header` (Origin, Sec-Fetch-*), `cors_never_allows_the_native_session_header` |
| Stale native cookie jar | `a_stale_cookie_never_overrides_the_native_session`. Native clients have no jar, so the server replaces any Cookie header with the header value |
| Device with no push (GPUI, NSH desktop) | `register_*` tests in `device_registry.rs` (register without a token). Client behaviour: manual |
| Many users behind one carrier IP | `ip_ceiling_is_at_least_ten_times_the_account_ceiling`. Device register and challenge are keyed on the account first (`authenticated_write_limits`) |
| Sign-out, then old value | `logout_ends_a_native_session_at_once` |
| One NSH client on phone and desktop | `nsh_spans_mobile_and_desktop`: every NSH platform accepted, `web` and a `browser` refused, store labels per the register table |
| Device removed (revoke) | Full flow test: delete self, next call 401 |

### Conformance (every client)

A client is ready when it passes, from `contracts/`:

- `uuid-v7.json`
- `sync-vectors.json` (and `sync.md`)
- `device-vectors.json`
- the O32 checklist below

### Manual checks (O32, need a real device or a live provider)

1. iOS and Android: the app keeps `x-eq-session` in the Keychain or Keystore, and never writes it to logs or analytics.
2. Native session across app kill and restart, on a device, after a real idle period.
3. Lost rotation response on a device: the next call 401s and the client signs in again without losing dirty rows.
4. React Native: no cookie store is used and `credentials` is `omit`. NSH: no cookie store is used.
5. GPUI and NSH desktop: no cookie store is used. Sign-in through the system browser with PKCE and a loopback redirect, for each provider whose desktop rules were checked.
6. Passkey on iOS (Associated Domains) and on Android (Play App Signing hash).
7. Sign in with Apple, next to the other social sign-in options (O31).
8. APNs delivery of the alert block with the production APNs key (O34).
9. Device lifecycle rows from D3 (reinstall, backup restore, device-to-device transfer) and the push rows in the edge ledger that need a device.
| Device with no push (GPUI) | No token; local notifications |
| Push token rotates or goes stale | Re-uploaded on refresh; terminal errors prune it |
