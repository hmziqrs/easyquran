# Device contract

What every client must do for account devices.

## Edge cases

| Case | Required behaviour |
| --- | --- |
| More than 20 devices | Oldest idle evicted, never the caller |
| Sign-out on a shared phone | Push token deleted first |

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

Fields added to `POST /device/v1/register`:

| Field | Rule |
| --- | --- |
| `key_store` | Required. Web: `browser` or `none`. React Native: `secure_hardware` or `none`. GPUI: `os_keystore`, `file` or `none` |
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
