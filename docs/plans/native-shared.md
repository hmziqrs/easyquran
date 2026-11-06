# Native apps: shared foundations

[Back to index](../README.md)

Work both apps need. The apps build on the contracts from [sync.md](sync.md) and [devices.md](devices.md), and do not invent their own.

#### N1. Repo layout and shared client core

Build:

1. Mobile lives at `apps/mobile`, added to the root bun workspace. Desktop lives at `apps/desktop` as its own Cargo workspace, never in `rust/Cargo.toml`, so server builds and CI do not compile GPUI.
2. Move the pure client logic out of `web/src/lib/sync` into `packages/client-core`, used by web and mobile: ids, clock, merge, wire types, status mapping. No Svelte, DOM or storage imports. Storage and transport are injected.
3. Move the Rust merge core to `rust/crates/eq-sync-core`, used by the API and by desktop through a path dependency. It also holds ids and the clock for desktop.
4. CI runs `contracts/*.json` vectors against all three implementations on any change to them.
5. Update the toolchain rule in `AGENTS.MD` for the new directories, and add load-on-demand context lines for them.

Files: root `package.json`, `web/vite.config.ts`, `web/src/lib/sync/*`, `rust/Cargo.toml`, new `packages/client-core/`, new `rust/crates/eq-sync-core/`, `.github/workflows/images.yml`, `AGENTS.MD`.

Done when: web and API behave the same after the move, all gates green; vectors pass through the shared packages; `cargo build` in `rust/` compiles no GPUI code.

Blocked by: S9

#### N2. Feature scope per client

Build: `contracts/feature-matrix.md`, one row per feature and one column per client, each cell `launch`, `later` or `web only`. Defaults, until the owner changes them (O33):

| Feature                                                      | Mobile                                                  | Desktop             |
| ------------------------------------------------------------ | ------------------------------------------------------- | ------------------- |
| Reader: surah, page, juz, hizb, Arabic scripts, translations | launch                                                  | launch              |
| Continue, History, Start over, progress                      | launch                                                  | launch              |
| Bookmarks and folders                                        | launch                                                  | launch              |
| Search and go-to-verse                                       | launch                                                  | launch              |
| Device list and remote sign-out                              | launch                                                  | launch              |
| Offline reading                                              | launch                                                  | launch              |
| Notes, device-local                                          | launch                                                  | launch              |
| Settings sync (F2), personal insights                        | launch                                                  | launch              |
| Push reminders                                               | launch                                                  | local notifications |
| Home cards, azkar                                            | later                                                   | later               |
| Audio, word-by-word, share image, voice search               | later                                                   | later               |
| Admin, moderation, posts, comments                           | web only                                                | web only            |
| Account deletion (F7)                                        | launch; stores require it in-app when sign-up is in-app | launch              |

Files: new `contracts/feature-matrix.md`.

Done when: every shipped web feature has a row; app items below name only `launch` rows.

Owner: O33.

#### N3. Quran data on native

Build:

1. Same data rules as web: immutable SQLite dumps by id, text verbatim from database to screen, no normalising or trimming, no SHA in any automated path. Bytes, size and path come from a baked id map generated from the same catalogue as `web/src/lib/data/translations.json`, never remote metadata.
2. The offline pack (`web/scripts/gen-offline-pack.ts`) stores web route responses, so native does not use it. Apps download the SQLite artifacts by id from the same storage web uses.
3. Bundle the default Arabic script so a first launch offline can read. Everything else downloads on demand, with progress, resume after interruption, a size check against the baked map, and an explicit storage screen.
4. Open databases read-only. A failed or partial download is discarded, never opened. A new text is a new id; nothing is ever patched.
5. Show the same attribution as web for each source. A source unknown to an older app is listed as unavailable; a thread's stored `source_id` is preserved (S3).
6. Conformance test: for a fixed verse sample per source, the app's read returns the same bytes as `/quran/v1`.

Files: new `contracts/quran-data.md`, `web/scripts/` (catalogue export), app data layers in M4 and G5.

Done when: both apps pass the byte conformance test; pulling the network mid-download leaves the app usable and the file absent.

Blocked by: N1
Owner: O35.

#### N4. Deep links and association files

Build:

1. `web/static/.well-known/apple-app-site-association` (`webcredentials` and `applinks`) and `assetlinks.json` (`delegate_permission/common.get_login_creds` and `handle_all_urls`). Serve as JSON, HTTP 200, no redirect, on the exact RP host. List the Play App Signing fingerprint, not only the upload key.
2. `contracts/routes.md`: the reader URL grammar, including `/t/` translation context and `?ayah=`. Apps parse it. An unknown path opens in the browser, never an error screen.
3. Desktop registers `easyquran://` for links and sign-in returns.

Files: new `web/static/.well-known/*`, `web/server.ts`, new `contracts/routes.md`.

Done when: header tests pass for both files; a shared web verse link opens the right verse and translation in each app; a passkey registers on Android (O32).

Blocked by: N2
Owner: O34.

#### N5. Analytics, crash reporting and privacy on native

Build:

1. Personal insights stay always on, through `user_analytics_v1` as on web, with events queued offline (insert-once).
2. Product analytics and crash reporting follow the same consent as web (A4), off until consent. No app-tracking prompt is needed while nothing tracks across other companies' apps; re-check if an SDK changes that.
3. No device id, hint or key in any analytics payload (standing rule). Audit each SDK's data use before adding it, because SDK behaviour counts against the app in Apple review.
4. Ship an Apple privacy manifest and fill App Privacy and Data Safety from the real SDK list (O31).

Files: app analytics modules, `contracts/devices.md`.

Done when: a network capture shows no analytics before consent and no device identifiers after it.

Blocked by: N2
