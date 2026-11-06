# Edge-case ledger

[Back to index](../README.md)

Every scenario we know of, its required behaviour, and who owns it. Finished rows move to `contracts/sync.md` (Sync) or `contracts/devices.md` (the rest). Accepted limits are deliberate; tell the owner before changing one.

### Clocks and time

| Case                                         | Required behaviour                                                                     | Item   |
| -------------------------------------------- | -------------------------------------------------------------------------------------- | ------ |

### Offline and conflicts

| Case                                                      | Required behaviour                                            | Item   |
| --------------------------------------------------------- | ------------------------------------------------------------- | ------ |

### Failures and crashes

| Case                                                      | Required behaviour                                                                            | Item       |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ---------- |
| Safari 7-day cap or site data cleared, with unsynced rows | Accepted limit: those local rows are lost. Session cookie survives, so the device row is kept | D3, O32    |
| Server restored from backup                               | `sync_epoch` forces full pull and re-push                                                     | S6, O2     |

### Accounts and sessions

| Case                                           | Required behaviour                                                                                                    | Item       |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ---------- |
| Account deleted on another device              | Next call 401. Local data stays until cleared, per the survives-logout policy. Non-dirty cache dropped after 180 days | S7, F7     |
| Minor flag set                                 | No device row; random stamp tail; existing rows deleted                                                               | D2, A34    |

### Device identity

| Case                                                       | Required behaviour                                            | Item        |
| ---------------------------------------------------------- | ------------------------------------------------------------- | ----------- |
| iOS reinstall                                              | Keychain survives: same row                                   | D3, O32     |
| Android reinstall                                          | Hint matches: same row                                        | D3, O32     |
| Backup restored onto a new phone or Mac                    | `ThisDeviceOnly` and hardware keys absent: new row            | D3, O32     |
| Android device-to-device transfer                          | Identity excluded by `dataExtractionRules`; new row           | D3, M3, O32 |
| Push token rotates or goes stale                           | Re-uploaded on refresh; terminal errors prune it              | D2, A36     |
| Device with no push (GPUI)                                 | No token; local notifications                                 | D6, F9      |

### Versions and platforms

| Case                                          | Required behaviour                                                 | Item     |
| --------------------------------------------- | ------------------------------------------------------------------ | -------- |
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
