# Mobile app: React Native

[Back to index](../README.md)

iOS and Android. Build only `launch` rows in `contracts/feature-matrix.md` (N2).

#### M1. App scaffold

Build: Expo with a development client unless the owner decides otherwise (O33), in `apps/mobile`; bun workspace with Metro configured for its symlinks; TypeScript at the repo's strict settings; lint, typecheck and test gates; navigation with the N4 route grammar; forced RTL handling for Arabic UI via `I18nManager`, including the restart it needs. New Architecture on. Pin versions like the rest of the repo.

Files: new `apps/mobile/`, root `package.json`, `.github/workflows/images.yml`.

Done when: a clean clone builds iOS and Android debug apps, gates run in CI, and web's gates are unaffected.

Blocked by: N1, D6
Owner: O33, O34.

#### M3. Session and device identity

Build: the D6 `x-eq-session` transport with `credentials: 'omit'`; session value in Keychain or Keystore-wrapped storage; CSRF handling; `crypto.getRandomValues` available before any id is minted (polyfill if the runtime lacks it). A small native module for the D3 key: Secure Enclave and Android Keystore P-256, DER to raw conversion, no user-auth flags, foreground only. Android `dataExtractionRules` excluding identity from both `cloud-backup` and `device-transfer`. `AfterFirstUnlockThisDeviceOnly` on iOS. A first-run check clears nothing: reinstall keeps the Keychain identity on purpose. Sign-in screens per the D6 matrix, including TOTP and Sign in with Apple. The `verified` guard state: unverified accounts see a verify step.

Files: `apps/mobile/src/auth/`, `apps/mobile/modules/device-key/`, `android` and `ios` config.

Done when: `contracts/devices.md` vectors pass on device; reinstall keeps one row on iOS and Android; a restore onto another phone makes a new row (O32).

Blocked by: M1, N4

#### M4. Local store and sync

Build: one SQLite file per account plus one for signed-out data, schema versions with forward-only migrations (S8), read-only Quran databases (N3), the `client-core` merge and clock, dirty rows (S7), foreground and resume polling (S6), and a sync status surface. Writes commit before the UI confirms. No sync in background is assumed.

Files: `apps/mobile/src/store/`, `apps/mobile/src/sync/`.

Done when: all shared vectors pass on device and in CI; the S9 scenarios pass against this client's store with a fake server; killing the app at each step loses nothing.

Blocked by: M3, N3

#### M5. Reader

Build: `launch` reader scopes and scripts with the bundled fonts and per-font leading (Nastaliq needs about 2.2–2.9×), translations, position restore by `surah:ayah`, read tracking with the same heuristics as web and no penalty for very fast readers, thread entry rules from `web/src/lib/sessions/entry.ts`, Start over. Large-text support, list virtualisation, phone and tablet layouts, foldables and rotation keep position.

Files: `apps/mobile/src/reader/`.

Done when: a thread started on web continues on the phone at the same verse and back; a 286-ayah surah scrolls without dropped position after rotation; read events match web for the same reading (fixture).

Blocked by: M4
Owner: O35 for font licences.

#### M6. Yours, bookmarks, search, settings, devices

Build: `launch` rows: Continue, History, bookmarks and folders with bulk actions if F14 has shipped, search and go-to-verse, settings, device list and remote sign-out, account deletion, data storage screen. Silent safe defaults with toasts, as on web.

Files: `apps/mobile/src/screens/`.

Done when: each row behaves as on web, tested against the same API; conflict cases in the edge-case ledger pass.

Blocked by: M5

#### M7. Push and reminders

Build: FCM on Android and iOS through APNs; permission asked in context, never at launch, with a denied state; token upload on refresh and weekly at most; token deleted before sign-out; deep link from a notification. Reminders from F9 when it has shipped.

Files: `apps/mobile/src/push/`.

Done when: a test notification arrives on real devices, opens the right verse, and stops after sign-out.

Blocked by: M6
Owner: O34.

#### M8. Localization and text size

Build: `en` and `ar`, sharing message keys with web where they match; RTL throughout; Dynamic Type and Android font scale; contrast from the design tokens; reduced motion. Screen-reader support is out of scope by owner decision.

Files: `apps/mobile/src/i18n/`, all screens.

Done when: a full reader, bookmark and sign-in pass works in Arabic at the largest text size.

Blocked by: M6

#### M9. Release

Build: signing and Play App Signing, store listings and screenshots, privacy manifest, App Privacy and Data Safety, version and protocol reporting (S8), forced-update prompt that leaves reading available, staged rollout. OTA updates only for JavaScript and assets, within the current protocol, and never to change store-reviewed purpose.

Files: `apps/mobile/app.config.*`, `.github/workflows/`, `deploy/README.md`.

Done when: a signed build passes O32, and an old build meets the 426 path correctly.

Blocked by: M7, M8, N5
Owner: O31, O32, O34.
