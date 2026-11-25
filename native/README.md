# EasyQuran native

The EasyQuran app for macOS, iOS and Android (Linux and Windows from the same crate), built on [Native Shell](https://nsh.dev) 0.7.1 in Rust mode with SuperNSH crates from their GitHub repositories, pinned by commit in `Cargo.toml`. `nsh run` and `nsh package` need an `nsh` CLI of 0.7.1. It is the native counterpart of `web/` and aims at 1:1 feature parity.

- What must be built, and its state: [PARITY.md](PARITY.md)
- Rules, layout and gates for anyone working here: [AGENTS.md](AGENTS.md)
- The shell, UI wrappers and foundation started from the MIT-licensed nsh-starter: [LICENSE.nsh-starter](LICENSE.nsh-starter)

## Layout

- `crates/eq-core`: domain logic in plain Rust (no NSH), tested with `cargo test -p eq-core`.
- `src/`: the NSH app crate: shell, UI wrappers, services over NSH, stores, feature screens.

## Run

```sh
cargo run                       # desktop, on this machine
nsh run --target ios            # iOS simulator
nsh run --target android        # first adb device
```

Quran databases are provisioned into `assets/quran/` out of band and are never committed.

## Provisioning: sign-in, push, analytics

Nothing below is committed. Every value is read at build time (`option_env!`); a missing value hides the feature, and the app builds and runs without any of them.

### Social sign-in (`nsh-auth`)

The provider token goes to the API's `POST /auth/{provider}/v1/token` for sign-in and `POST /auth/{provider}/v1/token/link` for fresh-auth account linking (GitHub: the code goes to `/auth/github/v1/native/exchange` or `/auth/github/v1/native/link`, below). Each client id must also be an audience or app the API accepts (its env names in brackets).

| Env (build time) | Platform | Provider console | Redirect to register |
| --- | --- | --- | --- |
| `EQ_GOOGLE_DESKTOP_CLIENT_ID` (+ `EQ_GOOGLE_DESKTOP_CLIENT_SECRET`, Google's non-secret installed-app secret) | macOS, Linux, Windows | Google Cloud, OAuth client type "Desktop app" [`GOOGLE_DESKTOP_CLIENT_IDS`] | none: `http://127.0.0.1:<port>` loopback, any port |
| `EQ_GOOGLE_IOS_CLIENT_ID` | iOS | OAuth client type "iOS", bundle id `fyi.easyquran.app` [`GOOGLE_MOBILE_CLIENT_IDS`] | none: Google's reversed client id scheme `com.googleusercontent.apps.<prefix>:/oauth2redirect` |
| `EQ_GOOGLE_ANDROID_CLIENT_ID` | Android | OAuth client type "Android", package `fyi.easyquran.app` + signing SHA-1, "Custom URI scheme" enabled under Advanced settings [`GOOGLE_MOBILE_CLIENT_IDS`] | none: `fyi.easyquran.app:/oauth2redirect` |
| `EQ_DISCORD_CLIENT_ID` | iOS, Android | the API's own Discord application [`DISCORD_CLIENT_ID`], with "Public Client" on so the PKCE exchange needs no secret | `fyi.easyquran.app:/oauth/discord` |
| `EQ_APPLE_SERVICES_ID` | all | Apple Developer, a Services ID with Sign in with Apple on [`APPLE_CLIENT_ID`, or listed in `APPLE_MOBILE_CLIENT_IDS`] | Return URL `<relay base>/auth/apple/v1/native/relay` |
| `EQ_FACEBOOK_APP_ID` | all | the API's Facebook app [`FACEBOOK_CLIENT_ID`], Facebook Login with PKCE | Valid OAuth Redirect URI `<relay base>/auth/facebook/v1/native/relay` |
| `EQ_GITHUB_CLIENT_ID` | all | a GitHub OAuth app for the app [`GITHUB_NATIVE_CLIENT_ID` + `GITHUB_NATIVE_CLIENT_SECRET` on the API] | Authorization callback URL `<relay base>/auth/github/v1/native/relay` |
| `EQ_OAUTH_RELAY_BASE` | all | the public API base the relays above are registered under; default `https://easyquran.fyi/api` | none |

Apple, Facebook and GitHub accept only HTTPS redirects, so they redirect to the API's relay, which answers `302` to the fixed `fyi.easyquran.app:/oauth/<provider>` link; the app takes it back through NSH's `open_auth_session` (desktop: the system browser and the claimed scheme). The app checks `state` itself. Apple's `form_post` carries the id token, which goes to `/auth/apple/v1/token`; Facebook's code is exchanged in the app (PKCE, no secret); GitHub's code and PKCE verifier go to the API's broker `POST /auth/github/v1/native/exchange`, which holds the secret. On desktop the scheme link only comes back where the scheme is registered: the packaged macOS app, the Linux runner (it writes the `.desktop` entry) and the Windows release runner (the Windows development host registers nothing). Not offered: passkeys (no library). iOS offers no social sign-in unless Apple is configured (App Store rule, `contracts/devices.md` O31). The callback scheme `fyi.easyquran.app` is claimed in `nsh.json` `schemes`.

### Firebase: push, analytics, crash reporting (`nsh-firebase`)

- **Analytics and crash reports** use the GA4 Measurement Protocol (pure tier). Build with `EQ_GA_MEASUREMENT_ID` (the web stream's `G-…` id), `EQ_GA_API_SECRET` (GA4 Admin → Data streams → Measurement Protocol API secrets) and optionally `EQ_FIREBASE_PROJECT_ID`. Nothing is sent until the user turns analytics on in Settings; crash reports are the `exception` event, under the same consent.
- **Push (FCM)** needs the library's native tier: the `firebase.*` handlers of `native/ios/NshFirebase` and `native/android` in the nsh-firebase repository, registered by an embedding host. An `nsh build` app has no host channel for them yet, so push is off and devices register without a token. When a host exists, its config files are provisioned out of band, never committed (both are gitignored):
  - iOS: `GoogleService-Info.plist` added to the app bundle as a resource (`Bundle.main` → `FirebaseApp.configure(options:)`), the APNs key uploaded to the Firebase project, the Push Notifications capability on, `FirebaseAppDelegateProxyEnabled = false` in Info.plist.
  - Android: `google-services.json` copied to the app's `assets/` (read at start by `NshFirebase.init`; no Gradle plugin), `POST_NOTIFICATIONS` in the manifest.
- Desktop has no push (devices.md push matrix): reminders use local notifications. Packaged macOS and iOS builds schedule daily local reminders via `UNCalendarNotificationTrigger`; other platforms fall back to the in-app reminder timer while running.
- Universal Links / App Links (`links`) and Apple Handoff (`handoff`) are declared in `nsh.json`, and the shell publishes the current canonical `https://easyquran.fyi` path on route changes.

## Gates

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just check` and `just test` run the same.
