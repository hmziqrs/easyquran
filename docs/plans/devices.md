# Devices

[Back to index](../README.md)

Device registry, fingerprint and continuity, device UI, handoff and the native client contract.

Account device management for web, React Native and GPUI desktop. Not an abuse signal; Gate B still governs that. Sync correctness never depends on recognising a device: at worst a duplicate shows in the list.

#### D3. Device fingerprint and continuity

The fingerprint is a random key only that install holds, plus a salted OS id on the platforms where the key alone cannot survive a reinstall. Enough to keep one device as one row, and nothing more. Everything here is optional at runtime: with no key and no hint a device still reads, syncs and appears in the list.

Build:

1. The client creates one P-256 key pair per account per install: the one curve Secure Enclave, Android Keystore, WebCrypto and desktop stores all support. The fingerprint is its RFC 7638 JWK thumbprint (SHA-256, base64url, members `crv`, `kty`, `x`, `y`).
2. Signature wire format is raw `r||s`, 64 bytes, base64url. WebCrypto already produces this; iOS and Android produce DER and must convert. Server verifies with `aws-lc-rs` (`ECDSA_P256_SHA256_FIXED`), already in the lockfile through `jsonwebtoken`. Vectors cover both inputs.
3. Android, Windows and Linux also send a continuity hint: `HMAC-SHA256(per-account salt, OS id)`, with the OS id lower-cased and trimmed. The raw OS id never leaves the device. The server generates the 32-byte salt per user and returns it with the challenge, so hints from two accounts can never match.
4. Per platform:

   | Client          | Key and id stored in                                                                                                                                                                                                                                                                                 | `key_store`             | Hint source                                                                                      | App reinstall               | Backup restored on another device | Other identity changes                                                                                          |
   | --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ------------------------------------------------------------------------------------------------ | --------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------- |
   | Web browser tab | Key: non-extractable WebCrypto in IndexedDB. Id: `localStorage`. Call `navigator.storage.persist()`                                                                                                                                                                                                  | `browser`               | None                                                                                             | n/a                         | n/a                               | Each browser, profile and installed PWA is its own device. Site data cleared or Safari's 7-day cap: rung 2 or 4 |
   | iOS             | Secure Enclave key, no user-presence flag. Id: Keychain, `AfterFirstUnlockThisDeviceOnly`                                                                                                                                                                                                            | `secure_hardware`       | None                                                                                             | Keychain survives: same row | Key absent: new row               | A found public key with a failing private key counts as lost                                                    |
   | Android         | Keystore key, no user-auth binding (lock-screen changes would destroy it); StrongBox when present, else TEE. Id in app storage, excluded by `dataExtractionRules` from both `cloud-backup` and `device-transfer`, because `allowBackup=false` does not stop device-to-device transfer on Android 12+ | `secure_hardware`       | `ANDROID_ID` (Play has not treated it as a persistent identifier since April 2025; O31 rechecks) | Rung 3: same row            | Key absent: new row               | Factory reset and signing-key change alter the hint: new row. Each user and work profile is its own device      |
   | macOS           | Data-protection Keychain, `ThisDeviceOnly` (needs a signed build); unsigned dev builds use the file fallback                                                                                                                                                                                         | `os_keystore`           | None                                                                                             | Keychain survives: same row | Item absent: new row              |                                                                                                                 |
   | Windows         | Credential Manager with DPAPI, local persistence, no roaming                                                                                                                                                                                                                                         | `os_keystore`           | `MachineGuid`                                                                                    | Same row                    | Rung 3 if the guid is unchanged   | Clones without Sysprep share a guid: see step 7                                                                 |
   | Linux           | Secret Service; if absent, locked or sandboxed (Flatpak, Snap, headless), a `0600` file under `$XDG_DATA_HOME`                                                                                                                                                                                       | `os_keystore` or `file` | `/etc/machine-id`, if readable                                                                   | Rung 3                      | Rung 3 if the id is unchanged     | Cloned VMs can share an id: see step 7                                                                          |

5. `POST /device/v1/challenge` returns a 32-byte nonce bound to the session, single use, 5-minute TTL (process-local, A13's no-eviction pattern; a server restart means the client asks again), and the salt. `register` sends the public key, optional hint, and a signature over the UTF-8 lines `easyquran-device-v1`, nonce, device id, user id. Key use happens only here, in the foreground, never during sync, because hardware keys can fail while a phone is locked. The migration adds `public_key`, `key_thumbprint`, `key_store`, `continuity_hint`, `hint_conflict` and `merged_key_thumbprint` to `user_devices`, plus a `user_device_salts` table following the standing table rules.
6. Recognition, first match wins, always within one account:
   1. Known id, valid key, hint matches or absent: same row.
   2. Local key or id lost, current session already bound to a row: same row, new key, and the client takes the row's id. Holding the session is the proof.
   3. New id, hint matches a live row with no `hint_conflict`: same row, new key. Save the old id and key thumbprint through `merged_into`. Do not revoke the old sessions; they expire by themselves.
   4. Otherwise a new row. Any older row with the same `platform`, `client`, `browser` and `form_factor` and no live session gets `retired_at`. If that device returns with its valid key, clear `retired_at`.
7. Shared hints are detected, never trusted. If an install presents a valid old key that was merged under rung 3, two live machines share that hint: restore the old row, set `hint_conflict` on both rows, and never match on them again. Neither machine is signed out.
8. Known id with a wrong key, or a valid key with a different hint: 409. The client mints a new id and key and registers again, keeping local rows and dirty data. Old stamps stay valid, since stamps only rank.
9. Accepted limit: a full disk clone with keys intact looks like one device on any platform. Sync stays correct, and the list shows one row. Removing it signs out both.
10. No key available (private window, insecure origin, WebCrypto or IndexedDB failure, keystore error): register with `key_store: none`. Recognition uses rung 2 and 4 only. Never block on it.
11. First-run race: tabs create the key with an add-if-absent IndexedDB transaction and read back the winner. A mismatch between stored id and key counts as lost.
12. Never merge two devices on coarse traits alone. A wrong merge hides a real device; a duplicate row only looks untidy.
13. `key_store` is a self-reported display label. App Attest, DeviceCheck and Play Integrity are out of scope. The key and hint never unlock anything. A non-extractable browser key still works for any script on the origin, which is why.

Files: `rust/backend/api/src/modules/device_v1/*`, `rust/backend/api/Cargo.toml`, new migration, `web/src/lib/device/`, `contracts/devices.md`.

Done when: every rung, 409 case, shared-hint recovery and the no-key path have tests; wrong signature, reused nonce and expired nonce fail; the same machine under two accounts yields different hints; DER-to-raw, thumbprint and hint vectors in `contracts/devices.md` pass in web and Rust; two tabs racing first sign-in end with one key.

Blocked by: D2
Owner: O31 before any store release.

#### D4. Devices on the account page

Build: replace the Sessions card on `/account` with Devices: name, platform icon, browser or OS, last seen, "This device". Rename inline. "Sign out" on another device acts at once with a toast, no dialog. On this device it runs normal sign-out and keeps the id. Sessions with no device show as "Unknown device" using the existing fixed-table label. Strings go in the `account` namespace (`en`, `ar`).

Files: `web/src/routes/(account)/account/+page.svelte`, `web/src/routes/(account)/account/account-view.ts`, `web/src/lib/auth/account-client.ts`, `web/messages/account/{en,ar}.json`.

Done when: list, rename and remote sign-out work in both locales; labels never echo unvalidated input (test); `bun run check` stays warning-free.

Blocked by: D2, U1

#### D5. Handoff by device

Build:

1. `device_id` in the thread's cursor register. An unknown or revoked id is stored as sent but resolves to no name, so threads started offline still land.
2. Pull rows return `deviceName`, resolved through `merged_into` from the user's own rows.
3. `ContinueThreads` compares device ids instead of device kinds, so two phones are no longer "the same". Label: "on Pixel · 5 min ago". Falls back to the kind label when no name resolves.
4. F6 export leaves out `device_id` and stamps. Import only adds rows whose ids are absent and stamps those fresh, so an old export can never overwrite newer data.

Files: `rust/backend/api/src/modules/reading_session_v1/*`, `rust/backend/api/src/db/sea_models/reading_session.rs`, `web/src/lib/sessions/{schema.ts,store.svelte.ts}`, `web/src/routes/(application)/yours/_components/ContinueThreads.svelte`.

Done when: a thread from another phone shows that phone's name; removing the device falls back to "on your phone"; a reinstalled Android device keeps its label; S9 covers a revoked device whose dirty rows land after re-sign-in.

Blocked by: D3, S9

#### D6. Native client contract and session transport

Native sign-in pieces exist (`/auth/{provider}/v1/token`, `ensure_native_token_request`), but the API has no bearer tokens, and React Native's cookie jar has reported problems: stale cookies overriding fresh ones, manual `Cookie` headers appended rather than replaced, and differences between iOS and Android. So native clients do not use a cookie jar. A thin adapter keeps every session, CSRF, rotation and revocation rule unchanged.

Build:

1. Middleware outside the session layer. A request with `x-eq-session` and no `Origin` is native: copy the header value in as `Cookie: ruxlog.sid=<value>`, and an empty value means no session yet. On its response, move the `ruxlog.sid` `Set-Cookie` to the `x-eq-session` response header (empty means cleared), and remove all `Set-Cookie`. A request with both `Origin` and `x-eq-session` is refused. The header is not added to the CORS allow list, so browsers cannot send it cross-origin. It never reaches logs, traces or error reports.
2. Clients send `credentials: 'omit'` (React Native) or use no cookie store (GPUI), and keep the value in the D3 secure store. The value is the same encrypted cookie, so it has the same secret class.
3. `contracts/devices.md`, session section:
   - Base URL includes `/api` in production.
   - Fetch `csrf-token` from `/csrf/v1/generate`. It derives from the session id, so refetch it after any `x-eq-session-rotated: 1` and any new session.
   - A lost rotation response means 401 on the next call and a new sign-in. Dirty rows wait.
   - Handle `pending_login` (TOTP required) on every sign-in path, and the unverified state on `verified` routes.
4. Session lifetime: sessions expire after 14 days of inactivity (`main.rs`). A native session registered by a device with a verified key gets 180 days through `Session::set_expiry`. Web stays 14. Revoke and bans still end them at once.
5. Sign-in matrix in the contract, each with a test or manual check:

   | Method             | iOS                                                                    | Android                                                                                                                | GPUI desktop                                                                                                                                                                                        |
   | ------------------ | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | Email and password | Native form                                                            | Native form                                                                                                            | Native form                                                                                                                                                                                         |
   | Google and others  | Provider SDK, then `/token`                                            | Provider SDK, then `/token`                                                                                            | System browser with PKCE and a `127.0.0.1` loopback redirect, then `/token`. Add desktop client ids next to `GOOGLE_MOBILE_CLIENT_IDS`. Each provider only after checking its current desktop rules |
   | Sign in with Apple | Required alongside other social sign-ins (Guideline 4.8; O31 confirms) | Optional                                                                                                               | Optional                                                                                                                                                                                            |
   | Passkey            | Associated Domains on the existing RP (N4)                             | Allow `android:apk-key-hash:<hash>` in `services/webauthn.rs`, for the Play App Signing key; `assetlinks.json` from N4 | Not at launch                                                                                                                                                                                       |

6. Push matrix: FCM on iOS (APNs key, O34), Android and web. `rux-fcm` sets no `apns` block today; add it. Windows and Linux have no FCM client, and macOS push is not at launch, so GPUI registers no token and uses local notifications. The list must not imply push exists on such a device.
7. Mobile carrier NAT shares one IP across many users, so authenticated limits must key on the user first (A6).
8. Integration test acting as a native client, no `Origin` and no cookies: fetch CSRF, sign in, register with key proof, sync a v7 thread, rotate, refetch CSRF, list devices, delete self, get 401. A second test proves a browser-shaped request cannot use the header.
9. Check every synced route works without browser-only behaviour. Fix what doesn't.
10. The contract includes the conformance list: a client is ready when it passes `uuid-v7.json`, `sync-vectors.json`, the device vectors, and the O32 checklist.

Files: `contracts/devices.md`, new `rust/backend/api/src/middlewares/native_session.rs`, `rust/backend/api/src/main.rs`, new `rust/backend/api/tests/native_client.rs`, `rust/backend/api/src/services/auth.rs`, `rust/backend/api/src/services/webauthn.rs`, `rust/backend/api/crates/rux-fcm/src/client.rs`.

Done when: the tests pass on `cargo test`; a native session survives 30 idle days in a test and a web one does not; web sign-in is unchanged; a React Native or GPUI developer can build sign-in, device registration and sync from `contracts/sync.md`, `contracts/devices.md` and the vectors alone.

Blocked by: D3, D5
Owner: O31, O32.
