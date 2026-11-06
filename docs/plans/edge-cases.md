# Edge-case ledger

[Back to index](../README.md)

Every scenario we know of, its required behaviour, and who owns it. Finished rows move to `contracts/sync.md` (Sync) or `contracts/devices.md` (the rest). Accepted limits are deliberate; tell the owner before changing one.

### Clocks and time

| Case                                         | Required behaviour                                                                     | Item   |
| -------------------------------------------- | -------------------------------------------------------------------------------------- | ------ |
| Device clock hours behind                    | Edit after seeing a row beats that row. Before any sync, it may lose, and it converges | S2, S3 |
| Device clock hours or a year ahead           | Rewritten to server + 60 s, `last` reset, converges                                    | S2, S3 |
| Time-zone change or travel                   | Local day follows the user's zone; stamps are unaffected                               | S2, F8 |

### Offline and conflicts

| Case                                                      | Required behaviour                                            | Item   |
| --------------------------------------------------------- | ------------------------------------------------------------- | ------ |
| Same thread moved on two devices                          | Highest cursor stamp wins; progress keeps the MAX             | S1, S3 |
| Same scope started on two devices offline                 | Two threads. Continue shows the newest                        | S1, S3 |
| Thread ended on A, read further on stale B                | Stays ended; B's cursor lands; B's reading keeps its progress | S3     |
| Deleted on A, read further on stale B                     | Stays deleted; B starts a new thread on its next read         | S3     |
| Delete then Undo, with another device offline             | Later stamp restores; other device converges                  | S3     |
| Folder renamed on A, reordered on B                       | Both survive                                                  | S1, S4 |
| Folder deleted on A, bookmark added on B later            | Bookmark moves to the default folder                          | S1, S4 |
| One verse bookmarked on two devices offline in one folder | One row survives on every client                              | S1, S4 |
| Same folder name created twice offline                    | Both kept                                                     | S1, S4 |
| Two settings changed on two devices                       | Both survive                                                  | S1, F2 |
| Device offline longer than 180 days                       | Full pull; nothing deleted revives; its own newer rows land   | S6     |
| 10,000 offline scrolls                                    | One dirty row                                                 | S7     |
| Very large first sync or import above the daily ceiling   | Stays dirty, continues next day, surfaced; pulls still work   | S5, A7 |

### Failures and crashes

| Case                                                      | Required behaviour                                                                            | Item       |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ---------- |
| App killed after row persisted                            | Nothing lost                                                                                  | S7         |
| Hard kill during cursor debounce                          | Accepted limit: last moments of scroll position; lifecycle writes never lost                  | S7         |
| Response lost after server commit                         | Replay changes nothing                                                                        | S1, S7     |
| Duplicate or reordered requests                           | No effect                                                                                     | S1, S9     |
| One invalid row in a batch                                | Only that row is refused                                                                      | S5         |
| Multi-page pull interrupted                               | Resumes; `since` saved after durable apply                                                    | S6         |
| Local storage full or failing                             | Degraded status, never reported as synced                                                     | S7         |
| Safari 7-day cap or site data cleared, with unsynced rows | Accepted limit: those local rows are lost. Session cookie survives, so the device row is kept | D3, O32    |
| Server restored from backup                               | `sync_epoch` forces full pull and re-push                                                     | S6, O2     |
| Server 5xx, deploy, `SQLITE_BUSY`                         | Backoff with jitter; rows stay dirty                                                          | S5         |
| Rate limited, many devices or tabs                        | One polling tab; `Retry-After` honoured                                                       | S5, S6, A6 |

### Accounts and sessions

| Case                                           | Required behaviour                                                                                                    | Item       |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ---------- |
| Session expires with dirty rows                | Rows wait; drain on sign-in to the same account                                                                       | S7         |
| Native app unused for weeks                    | 180-day session                                                                                                       | D6         |
| Second account signs in on one device          | Separate partition, id, key and hint; nothing crosses                                                                 | S2, D3, S9 |
| Signed-out reading, then sign-in               | Folds silently into that account, per product decision, even on a shared phone                                        | S2         |
| Account banned or email unverified during sync | Pause; rows kept; never dead-lettered                                                                                 | S5         |
| Remote sign-out while device is offline        | Next call 401; dirty rows wait; after sign-in a new id via 409                                                        | D2, D3, S9 |
| Account deleted on another device              | Next call 401. Local data stays until cleared, per the survives-logout policy. Non-dirty cache dropped after 180 days | S7, F7     |
| Same email re-registered                       | New user id. Accepted limit: the old partition's dirty rows never drain                                               | S7         |
| Minor flag set                                 | No device row; random stamp tail; existing rows deleted                                                               | D2, A34    |
| Session rotation on a native client            | `x-eq-session` replaced, CSRF refetched; lost response means sign in again                                            | D6         |
| Browser tries to use `x-eq-session`            | Refused; not in the CORS list                                                                                         | D6         |
| Stale native cookie jar                        | Impossible: native clients have no jar                                                                                | D6         |

### Device identity

| Case                                                       | Required behaviour                                            | Item        |
| ---------------------------------------------------------- | ------------------------------------------------------------- | ----------- |
| iOS reinstall                                              | Keychain survives: same row                                   | D3, O32     |
| Android reinstall                                          | Hint matches: same row                                        | D3, O32     |
| Backup restored onto a new phone or Mac                    | `ThisDeviceOnly` and hardware keys absent: new row            | D3, O32     |
| Keychain item visible but key unusable                     | Treated as lost                                               | D3          |
| Android lock screen removed                                | No user-auth binding, so key survives                         | D3          |
| Android device-to-device transfer                          | Identity excluded by `dataExtractionRules`; new row           | D3, M3, O32 |
| Phone locked during background sync                        | Sync uses no key                                              | D3          |
| Web IndexedDB or `localStorage` cleared, session alive     | Rung 2: same row                                              | D3          |
| Private window, insecure origin, WebCrypto failure         | `key_store: none`; still reads and syncs                      | D3          |
| Two browsers, or browser and installed PWA, on one machine | Separate devices, by design                                   | D3          |
| Two machines sharing `MachineGuid` or `machine-id`         | Detected on the second install; rows split; nobody signed out | D3          |
| Full disk clone with keys intact                           | Accepted limit: one row; sync still correct                   | D3, S9      |
| Android multi-user or work profile                         | Separate devices                                              | D3          |
| Tablet, foldable, or desktop-mode iPad changes form factor | Latest register wins; label follows the last write            | D2, D5      |
| Device removed while its threads remain                    | Label falls back to device kind                               | D5          |
| More than 20 devices                                       | Oldest idle evicted, never the caller                         | D2          |
| Sign-out on a shared phone                                 | Push token deleted first                                      | D2          |
| Push token rotates or goes stale                           | Re-uploaded on refresh; terminal errors prune it              | D2, A36     |
| Device with no push (GPUI)                                 | No token; local notifications                                 | D6, F9      |

### Versions and platforms

| Case                                          | Required behaviour                                                 | Item     |
| --------------------------------------------- | ------------------------------------------------------------------ | -------- |
| Old app or stale tab on a new server          | Works within a protocol; below minimum, 426 and reading continues  | S8       |
| Unknown enum, field or register               | Displayed neutrally and preserved                                  | S8       |
| App downgrade with newer local schema         | Store untouched; sync pauses                                       | S8       |
| Position written on another client or layout  | `surah:ayah` wins; page and ratio ignored unless same client class | S3       |
| Translation or snippet missing on this client | Display fallback; row value preserved                              | S3       |
| iOS or Android blocks background work         | Sync on foreground and resume; nothing depends on background       | S6       |
| Signature formats differ by platform          | Fixed 64-byte raw form on the wire; vectors enforce                | D3       |
| New client implementation                     | Must pass all shared vectors and O32                               | D6       |
| Many users behind one carrier IP              | User bucket governs; IP bucket at least 10×                        | A6, D6   |
| Microphone or audio blocked by browser policy | Headers and CSP updated with the feature                           | F15, F19 |

### Native apps

| Case                                  | Required behaviour                               | Item    |
| ------------------------------------- | ------------------------------------------------ | ------- |
| First launch with no network          | Bundled Arabic reads; downloads wait             | N3      |
| Download interrupted or file partial  | Discarded, resumes, never opened                 | N3      |
| Text differs from web                 | Conformance test fails the build                 | N3      |
| Web link for a path the app lacks     | Opens in the browser                             | N4      |
| Passkey fails on store builds only    | `assetlinks.json` lists the Play App Signing key | N4, O34 |
| Arabic UI needs RTL restart           | Handled once, position kept                      | M1      |
| App in background or killed by the OS | Nothing depends on background sync               | M4      |
| Rotation, foldable, window resize     | Position stays by `surah:ayah`                   | M5, G5  |
| Notification permission denied        | Clear state; app fully usable                    | M7      |
| Push after sign-out                   | Token deleted first; none arrives                | D2, M7  |
| Second desktop instance               | Focuses the first; one writer                    | G1      |
| Wake from sleep with a wrong clock    | Offset refreshes on sync                         | G4      |
| Tampered or failed desktop update     | Refused; user data untouched                     | G8      |
| Windows build from a cross compile    | Forbidden; built on Windows                      | G8      |
| Font not licensed for app bundles     | Face not shipped in the app                      | O35     |
| Reminder on phone and desktop         | Delivered by one, not both                       | F9, G6  |
