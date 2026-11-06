# Desktop app: GPUI

[Back to index](../README.md)

macOS, Windows and Linux on GPUI with HarfRust, starting from `experiments/gpui-kit-sample`. Build only `launch` rows in `contracts/feature-matrix.md` (N2).

#### G1. App scaffold

Build: `apps/desktop`, its own Cargo workspace; pinned `gpui-kit`; `rustfmt` and `clippy` gates; CI builds on macOS, Windows and Linux runners; the experiment's font and scroll findings (`experiments/gpui-kit-sample/README.md`) carried over as tests or comments. Single-instance lock; a second launch focuses the first and passes on its link.

Files: new `apps/desktop/`, `.github/workflows/`.

Done when: a clean clone builds and opens a window on all three systems in CI; `rust/` builds compile no GPUI.

Blocked by: N1, D6

#### G2. Production Arabic text element

Owner confirmed Arabic and Urdu render correctly on all desktop systems, so no rendering proof is needed. Selecting reading text is not required (owner decision, 2026-11-06), so the reading element needs no selection or caret.

Build:

1. Route all Arabic and Urdu reading text through HarfRust and `paint_glyph`. Never stock GPUI text.
2. Line breaking and wrapping for Arabic and Nastaliq, bidi for mixed text, per-font leading.
3. A copy-verse action, if one ships, reads verbatim text from the database, never from glyphs.
4. Editable fields (search, notes, rename, sign-in) are separate from this element. Use the `gpui-kit` input and confirm typed Arabic and Urdu display correctly in it.

Files: `apps/desktop/src/text/`.

Done when: wrapping and bidi cases pass at each launch font; copy-verse returns database bytes (test, if the action ships); typed Arabic and Urdu display correctly in a search field.

Blocked by: G1
Owner: O35 for font licences.

#### G3. Session and device identity

Build: the D6 `x-eq-session` transport with no cookie jar; the D3 stores per system, with the `file` fallback and its `0600` check; `hint` for Windows and Linux, none on macOS; signed macOS builds for the data-protection Keychain. Sign-in: email and password in the app; providers through the system browser with PKCE and a `127.0.0.1` loopback redirect on a random port (RFC 8252). Support each provider only after checking its current desktop rules; launch needs email and Google only. TOTP through `pending_login`.

Files: `apps/desktop/src/auth/`, `rust/backend/api/src/modules/{google,apple,facebook,github,discord}_auth_v1/*` only where a desktop client id is needed.

Done when: `contracts/devices.md` vectors pass; a reinstall keeps one row where D3 says it should; sign-in survives restart and rotation.

Blocked by: G1, N4

#### G4. Local store and sync

Build: one SQLite file per account plus signed-out data, migrations (S8), read-only Quran databases (N3), `eq-sync-core`, dirty rows, poll while focused and on wake. Wake from sleep refreshes the clock offset. Data under the OS app-data folder.

Files: `apps/desktop/src/store/`, `apps/desktop/src/sync/`.

Done when: shared vectors pass; S9 scenarios pass against this store with a fake server; kill and power-loss tests lose nothing already persisted.

Blocked by: G3, N3

#### G5. Reader

Build: `launch` scopes and scripts, translations (including side by side when width allows), position by `surah:ayah`, read tracking with the web heuristics and no penalty for very fast readers, window resize keeps position, HiDPI and multi-monitor scaling.

Files: `apps/desktop/src/reader/`.

Done when: a thread continues across web, phone and desktop at the same verse; long surahs scroll at display rate on a modest machine; read events match web for a fixture.

Blocked by: G2, G4

#### G6. Yours, bookmarks, search, settings, devices

Build: `launch` rows as on web, with a window, menu and keyboard model native to each system. Local notifications for reminders; no push token (D6). A desktop reminder fires only when the account has no mobile push token, so nobody is told twice.

Files: `apps/desktop/src/screens/`.

Done when: each row behaves as on web against the same API; ledger conflict cases pass.

Blocked by: G5

#### G7. Localization and keyboard

Build: `en` and `ar`, RTL layout, full keyboard operation, system text scale and contrast, reduced motion. Screen-reader support is out of scope by owner decision.

Files: `apps/desktop/src/i18n/`, all screens.

Done when: reader, bookmarks and sign-in work by keyboard alone, in Arabic.

Blocked by: G6

#### G8. Packaging and updates

Build: macOS signed and notarized bundle. Windows built on Windows with `fxc`, because dev cross builds read shaders from the build machine's path at runtime; signed installer. Linux AppImage first; Flatpak only after G3's sandbox path is tested. Signed auto-update with rollback that never touches user data or the keystore. Report version and protocol (S8); below the minimum, prompt to update and keep reading.

Files: `apps/desktop/packaging/`, `.github/workflows/`, `deploy/README.md`.

Done when: a signed build installs and updates on all three systems (O32); a tampered update is refused; an old build meets the 426 path correctly.

Blocked by: G7, N5
Owner: O32, O34.
