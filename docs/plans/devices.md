# Devices

[Back to index](../README.md)

Device registry, fingerprint and continuity, device UI, handoff and the native client contract.

Account device management for web, React Native and GPUI desktop. Not an abuse signal; Gate B still governs that. Sync correctness never depends on recognising a device: at worst a duplicate shows in the list.

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
