# Ids and sync

[Back to index](../README.md)

The multi-device conflict protocol for web, React Native and GPUI.

## Sync

Today, conflicts are decided by device wall clocks on whole rows, and clients infer deletes from a row being absent. These items replace that for three clients. Pre-launch, so replace in place with no shims.

#### S2. Clock and local device id

Build:

1. Mint the per-account device id locally at first sign-in (storage table in D3). No server row is needed yet. Signed-out edits carry no stamp; they get stamps when they fold in at sign-in, using their `updatedAt` as `ms`.
2. Every sync response returns `server_time`. The client saves `offset = server_time - local time`.
3. `stamp()` must be synchronous because reader writes are, so on web the id and `last` live in `localStorage`; only the key (D3) lives in IndexedDB. `ms = max(local time + offset, last stamp ms)`; same ms bumps the counter. Merging any received stamp advances `last`, so an edit made after seeing a row always beats that row, whatever the device clock says. Counter overflow bumps `ms`. Persist `last` per account before using it. Tabs share it and read it before each stamp. After any sync, if `last` is more than 60 s ahead of `server_time`, reset it to `server_time + 60 s`, so a clock once set a year ahead cannot poison later stamps.
4. Offline with a wrong clock and no offset yet: writes still work and still reconcile. They may lose to newer writes; they cannot dominate after the first sync.
5. `correctedNow()` (local time + offset) replaces `Date.now()` for staleness, labels and `localDay`. The user's time zone still decides the local day.

Files: new `web/src/lib/sync/clock.ts`, new `web/src/lib/device/identity.ts`, `rust/backend/api/src/modules/reading_session_v1/controller.rs`, `rust/backend/api/src/modules/bookmark_v1/controller.rs`.

Done when: unit tests against the merge core cover a device 1 h slow editing after a pull (its edit wins), a device 1 h fast after its first offset, clock moved backwards mid-session, and two tabs stamping at once.

Blocked by: S1

#### S3. Reading sessions on the protocol

Build: new migration adds `identity_stamp`, `cursor_stamp`, `lifecycle_stamp` to `reading_sessions`, with NULL ranking lowest. Replace `upsert_lww` and `delete_lww` with S1 merge: read, merge, write inside the sync transaction. A stamp more than 60 s ahead of `server_time` is rewritten to `server_time + 60 s`, keeping its device id; the client adopts it from the returned row. `updated_at` stays for display only. Local thread schema matches, and the client merges with S1. `reopen`, `dismiss`, `remove`, `startOver` and F4 undo write the lifecycle register; `cursor`, `anchor`, `noteRead` write the cursor register. Lists, staleness and the `/archive` keyset all use `cursor_stamp`, with an index, never `updated_at` or a raw device clock. `#openInScope` and Continue pick the newest per scope. `/archive` stays. `surah:ayah` is the position on every client; `local_page` and `ratio` are hints valid only when the writing device has the same `client` and `form_factor`, and otherwise ignored. An unknown `source_id` or `snippet:` scope falls back to display defaults and is preserved in the row.

Files: new migration, `rust/backend/api/src/db/sea_models/reading_session.rs`, `rust/backend/api/src/modules/reading_session_v1/*`, `web/src/lib/sessions/{schema.ts,store.svelte.ts,domain.ts}`.

Done when: a device 1 h fast converges with the server's row (today it keeps its own row indefinitely); an ended thread stays ended when another device's cursor arrives, and that cursor still lands; delete beats a stale reader; undo restores; progress never regresses; existing session tests are rewritten, not removed.

Blocked by: S2

#### S4. Bookmarks and folders on the protocol

Build: same reshape for `bookmarks` and `bookmark_folders`, following the table in S1, including the stamp rewrite. Keep the existing rival-key tombstone logic, ranked by stamp. Default folder stays UUID v5. Undo writes a newer stamp.

Files: new migration, `rust/backend/api/src/db/sea_models/bookmark/actions.rs`, `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs`, `rust/backend/api/src/modules/bookmark_v1/*`, `web/src/lib/bookmarks/{schema.ts,store.svelte.ts,domain.ts}`.

Done when: rename vs move of one folder on two devices both survive; folder delete vs newer bookmark follows the table; two ids for one key converge to one on every client; `tests/bookmarks_v1.rs` is rewritten to match.

Blocked by: S2

#### S5. Per-row sync results

Today one bad row fails the whole batch, and after 5 failures the engine dead-letters the batch head, which may be innocent.

Build:

1. Sync returns `results`: one per row, `{id, status, stamps?}`, where status is `applied | superseded | rejected | capped`. Validate rows one at a time. 400 only for a malformed request. The transaction still covers the round.
2. Engine: `rejected` dead-letters exactly that row and keeps its local copy; `capped` stays local and dirty and is surfaced; the rest proceed. Remove blind head eviction.
3. Client adopts any returned canonical stamp.
4. Status mapping, the same on all clients:

   | Response                                    | Client behaviour                                                                   |
   | ------------------------------------------- | ---------------------------------------------------------------------------------- |
   | 400 row `rejected`                          | Keep the row locally, stop resending until it is edited, surface the count         |
   | 401                                         | Sign-out state. Keep dirty rows for that account                                   |
   | 403 unverified, banned, or account mismatch | Pause. Never dead-letter. Keep dirty rows                                          |
   | 404 (auth nests unmounted)                  | Pause with backoff                                                                 |
   | 409 from device routes                      | Re-mint id and key (D3). Sync is unaffected                                        |
   | 426                                         | Pause sync, show update prompt, keep reading (S8)                                  |
   | 429                                         | Honour `Retry-After`. If it is the A7 daily ceiling, stop pushing but keep pulling |
   | 5xx, timeout, `SQLITE_BUSY`                 | Retry with backoff and jitter. Rows stay dirty                                     |

Files: `rust/backend/api/src/modules/reading_session_v1/controller.rs`, `rust/backend/api/src/modules/bookmark_v1/controller.rs`, `web/src/lib/sync/engine.svelte.ts`, `web/src/lib/sync/types.ts`, `web/src/lib/sessions/domain.ts`, `web/src/lib/bookmarks/domain.ts`.

Done when: a batch with one invalid row applies the others and dead-letters only that row (API and engine tests); a capped row survives locally and retries when room appears.

Blocked by: S3, S4

#### S6. Delta pull with tombstones

Today a client treats an open thread missing from the capped snapshot as ended elsewhere, and bookmarks reload the full list every round. Absence must stop meaning anything.

Build:

1. New migration: each synced row gets a server-assigned `rev` (threads, progress, bookmarks, folders), from a per-user, per-domain counter inside the write transaction.
2. Sync takes `since` and returns changed rows including tombstones, the new `rev`, and `more`. Page size 200. `since = 0` is a full pull. Pull rounds never count toward the A7 daily ceiling.
3. Client merges with S1 and deletes only on a tombstone. Remove the absence inference in `applyServer` and the local `#removed` ledger. Save `since` only after the page is durably applied.
4. Pull delivers a tombstone as id, stamps, state and `rev` only. On the server, content stays 24 h for Undo, then the sweeper scrubs it to neutral values (columns are `NOT NULL`) and a scrubbed row ignores further writes. Bookmark tombstones keep their key, which the rival-key logic needs. Tombstones are kept 180 days (A32). The response includes `tombstone_floor`. A client whose `since` is below the floor does a full pull. An upsert for an unknown id with a stamp below the floor returns `superseded`, so a device offline for a year cannot revive deleted rows.
5. Web polls only while visible, from one tab elected with `navigator.locks` (every tab polls if locks are missing). Native polls in the foreground and on app resume. Background sync is best effort: iOS and Android may never run it, so nothing depends on it. Push-triggered pull is out of scope.
6. Bootstrap: `since = 0` returns open threads, all progress, live bookmarks and folders, plus the current `rev`. History comes from `/archive`, so a 10,000-thread account stays cheap.
7. Server restored from backup: rows and `rev` go backwards. Every response carries `sync_epoch` (in `app_constants`; the restore runbook changes it, O2). If `sync_epoch` changed, or `since` exceeds the server's `rev`, the client does a full pull and marks every local row dirty. State rows make this safe, and it heals writes the restore lost.

Files: new migration, `rust/backend/api/src/db/sea_models/reading_session.rs`, `rust/backend/api/src/db/sea_models/bookmark/actions.rs`, `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs`, `web/src/lib/sync/engine.svelte.ts`, `web/src/lib/sessions/store.svelte.ts`, `web/src/lib/bookmarks/store.svelte.ts`.

Done when: delete on A reaches B past any page or cap; an interrupted multi-page pull resumes without loss; an empty response deletes nothing; a stale device past the floor recovers and revives nothing; a simulated restore converges with no acknowledged write lost from any device that still holds it.

Blocked by: S5

#### S7. The row is the queue

Today local rows and outbox entries are written separately, with the dirty set in memory. A kill between them leaves a row that was never queued, and the next pull deletes it.

Build:

1. Persist `dirty` on the local row, in the same write as the row. Lifecycle writes persist at once. Cursor writes may keep the current debounce with the `pagehide` flush, so a hard kill can lose only the last moments of cursor movement; state that limit in the contract. A sync round sends the current dirty rows. Clear `dirty` only when `results` acknowledges a stamp equal to the row's current stamps. 10,000 offline cursor moves become one row.
2. Row domains leave the outbox. It stays for insert-once data (reading events).
3. Dirty rows are never evicted by `LOCAL_THREAD_CAP` and never dropped at sign-out, session expiry or device revoke. They wait, per account, until that account signs in again.
4. If local storage fails or fills, surface the existing degraded status and keep the row in memory. Never report it synced.
5. Local storage per client: web keeps today's stores, and tabs merge with S1 on `storage` events, treating `dirty` as OR. React Native and GPUI use one SQLite file per account. GPUI takes a single-instance lock; a second window talks to the first.
6. Non-dirty cached rows of an account with no sign-in for 180 days are dropped on next launch, so orphaned partitions cannot grow forever. Dirty rows stay until that account signs in or the user clears app data.
7. Remove `#pendingThreads`, `#pendingDeletes` and the in-memory dirty sets. S1 merge makes overlays unnecessary.

Files: `web/src/lib/sessions/store.svelte.ts`, `web/src/lib/bookmarks/store.svelte.ts`, `web/src/lib/sync/engine.svelte.ts`, `web/src/lib/sync/outbox.ts`.

Done when: tests kill the app at every step after a row is persisted (mid-round, after server commit before ack) and lose nothing; a replay after a lost response changes nothing; expiry then re-sign-in drains dirty rows.

Blocked by: S6

#### S8. Protocol versioning and old clients

Web tabs stay open for days and phone apps cannot be force-updated, so old clients will talk to new servers for years.

Build:

1. Every sync body sends `client: {protocol, platform, app_version}`. No new headers, so the CORS table does not change.
2. Server keeps a minimum protocol per client. Below it: 426 with no row applied. Client pauses sync, shows an update prompt and keeps reading offline. Dirty rows wait and land after the update.
3. Additive changes only within a protocol: new fields and registers are optional. Clients preserve unknown fields on rows and display an unknown enum value (`end_reason`, `origin`, `device`) with a neutral default.
4. Server never removes a field or tightens a validator within a protocol. A breaking change means a new protocol and a period serving both.
5. Local storage: each store has a schema version and a forward-only migration. A store newer than the app (downgrade, sideload) is left untouched and sync pauses. `isFutureSchema` is the web pattern.
6. Contract tests: a frozen v1 request fixture must keep passing against every later server build.

Files: `contracts/sync.md`, `rust/backend/api/src/modules/reading_session_v1/*`, `rust/backend/api/src/modules/bookmark_v1/*`, `web/src/lib/sync/*`, new `rust/backend/api/tests/sync_contract.rs`, new `rust/backend/api/tests/fixtures/sync-v1/`.

Done when: an old-client fixture round-trips a row without losing a field it does not know; a client below the minimum gets 426 and applies nothing; an unknown enum renders without error (web test).

Blocked by: S7

#### S9. Multi-device simulator

Build: `rust/backend/api/tests/sync_sim.rs` drives the real HTTP routes with simulated clients built on the merge core. Seeded random runs: 2–5 devices, each with clock skew up to ±1 day; offline periods; lost responses; duplicated and reordered requests; a kill at any step; sign-out, expiry and re-sign-in; device revoke; two accounts on one device; a device offline past `tombstone_floor`; a server restore; an old-protocol client; a cloned device sharing an id. A failing seed prints a replayable trace. Fixed seeds run in `cargo test`; a long run goes in `just sync-soak`.

After every run, assert:

1. All devices hold identical rows after a final sync.
2. No acknowledged write vanished except by a rule in S1.
3. Progress never regressed.
4. No cursor write reopened or revived a thread.
5. No row crossed accounts.

Also add a web test running two real store instances against a fake server using the same vectors.

Files: new `rust/backend/api/tests/sync_sim.rs`, `justfile`, new `web/src/lib/sync/__tests__/two-devices.test.ts`.

Done when: fixed seeds pass; a deliberately broken merge rule makes the simulator fail (proven once, then reverted).

Blocked by: S8
