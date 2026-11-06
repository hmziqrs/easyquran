# Sync contract

The only spec a client needs to sync. Web, React Native and GPUI all follow it. Rules here are enforced by `rust/crates/eq-sync-core` and `web/src/lib/sync/merge.ts`. Both pass `contracts/sync-vectors.json`, and any new client must too.

## Stamps

A stamp is `{ms, 13 digits}-{counter, 4 lowercase hex}-{device id}`. The device id is 1 to 64 characters of `[A-Za-z0-9._-]`.

- Compare stamps bytewise. Higher wins.
- A stamp that fails the format is `rejected`.
- A NULL stamp (a row from before stamps) ranks lowest.
- On identical stamps, the register with the higher RFC 8785 canonical JSON bytes wins, so every replica picks the same result in every language.
- `ratio` is rounded to 4 decimals before it is stamped. Synced numbers are integers and those ratios only.

## Registers

A register is `{ "stamp": string | null, "value": any }`. Each row carries one per field group, and registers merge independently. A register absent from a write means no change, never a clear, so an older client cannot erase what it does not know about. Unknown registers keep merging by highest stamp. Rows are state, not operations, so replay and reordering are harmless.

## Merge rules

| Data | Register | Rule |
| --- | --- | --- |
| Thread | `identity`: `origin`, `originRef`, `scope`, `boundEnd`, `startedAt` | Set once. Lowest stamp wins |
| Thread | `cursor`: `surah`, `ayah`, `localPage`, `ratio`, `sourceId`, `device`, `deviceId` | Highest stamp wins. Moving back on purpose is allowed |
| Thread | `lifecycle`: `state` is `open`, `ended` (with `endedAt`, `endReason`) or `deleted` | Highest stamp wins. Only an explicit end, reopen, delete or undo writes it, so a stale reader can never reopen or revive a thread |
| Progress | Per surah | MAX of `furthestAyah`. Never regresses |
| Bookmark | `register`: `folderId`, `key`, `context`, `deleted` | Highest stamp wins. Two ids on one key: the lower stamp becomes a tombstone |
| Folder | `name`; `position`; `deleted` | Each highest stamp wins. The default folder cannot be deleted |
| Folder delete vs bookmark | Bookmark stamp compared to delete stamp | Older bookmarks die with the folder. Newer ones move to the default folder |
| Same folder name twice offline | None | Keep both. Never merge user-created data silently |
| Same scope on two devices offline | None | Keep both threads. Continue shows the newest per scope; the other stays reachable in History |
| Settings | One per setting key | Highest stamp wins, so two settings changed on two devices both survive |
| Home layout | Whole document | Highest stamp wins |
| Reading goals | Whole document under `goals.reading` | Highest stamp wins, same as the home layout. Progress is never stored |
| Reading bookmark | Whole value: `surah`, `ayah`, `sourceId`, `setAt`. Null when cleared | Highest stamp wins. Only an explicit set, replace, clear or undo writes it. Automatic resume never does |
| Reading events | Insert-once by id | Idempotent |

Rival-key and folder-delete rules run on the set of rows after merging (`convergeBookmarks`). They keep the row's stamp. At equal stamps `deleted: true` has the higher canonical bytes, so a tombstone made on one replica beats the live copy everywhere.

## Clock

Each account on each install keeps a hybrid logical clock.

- Every sync response returns `serverTime` in epoch ms. The client saves `offset = serverTime - local time`.
- `ms = max(local time + offset, last stamp ms)`. The same ms bumps the counter. Counter overflow bumps `ms`.
- Merging any received stamp advances `last`, so an edit made after seeing a row always beats that row.
- Persist `last` per account before using a stamp. Tabs share it and read it before each stamp.
- After any sync, if `last` is more than 60 s ahead of `serverTime`, reset it to `serverTime + 60 s`.
- Signed-out edits carry no stamp. They get one when they fold in at sign-in, using their `updatedAt` as `ms`.
- `now()` (local time + offset) replaces the raw device clock for staleness, labels and `localDay`. The user's time zone still decides the local day.

The device id is minted locally per account at first sign-in. No server row is needed.

## Wire format

`POST /reading-session/v1/sync` and `POST /bookmark/v1/sync`. JSON is camelCase.

Request:

| Field | Meaning |
| --- | --- |
| `expectedUserId` | Required. A mismatch is 403 before any write |
| `client` | `{protocol, platform, appVersion}`. In the body, so the CORS table never changes |
| `since` | Last `rev` applied. `0` is a full pull |
| `after` | Page cursor inside a full pull. Starts at `-1` |
| `mutations` | Up to 200 rows. Each parses alone, so one bad row is refused alone |

Rows:

- `session.upsert`: `id` (UUID v7), and any of three groups. `identityStamp` with `origin`, `originRef`, `scope`, `boundEnd`, `startedAt`. `cursorStamp` with `surah`, `ayah`, `localPage`, `ratio`, `sourceId`, `device`, `deviceId`. `lifecycleStamp` with `state`, `endedAt`, `endReason`. A new id needs identity and cursor.
- `progress.upsert`: `surah`, `furthestAyah`, `updatedAt`. Its result id is `progress:{surah}`.
- `folder.upsert`: `id`, then `nameStamp` with `name`, `positionStamp` with `position`, `deletedStamp` with `deleted`. A new id needs a name.
- `bookmark.upsert`: `id`, `stamp`, `folderId`, `surah`, `ayah`, `endSurah`, `endAyah`, `sourceId`, `deleted`. An unknown folder id files in the default folder.

`updatedAt` is display only. Stamps decide. A stamp more than 60 s ahead of the server is rewritten to `server + 60 s`, keeping its device id. The client adopts it from the result.

Response: `serverTime`, `syncEpoch`, `results`, `applied`, `superseded`, `rejected`, `capped`, the changed rows (`sessions` and `progress`, or `folders`, `bookmarks` and `defaultFolderId`), `rev`, `latestRev`, `more`, `tombstoneFloor`.

Each result is `{id, status, stamps?}`:

| Status | Client behaviour |
| --- | --- |
| `applied`, `superseded` with `stamps` | Adopt a lower returned stamp. Clear `dirty` only if the row's stamps are still the ones sent |
| `superseded` with no `stamps` | Older than the tombstone floor. The row may be long deleted: let it go |
| `rejected` | Keep the row, stop resending until it is edited, surface the count |
| `capped` | Keep it dirty, surface it, retry when room appears |

`POST /preference/v1/sync` carries settings, the home layout, the reading bookmark and reading goals. Rows are `{key, value, stamp}`, where `key` is `settings.*`, `home.layout`, `reading.bookmark` or `goals.reading`, and any other key is `rejected`.

### Reading goals

`goals.reading` holds `{ "goals": [...] }`, at most 10 goals. A goal is `{id, kind, dailyMinutes, startDay, endDay, pausedFrom, pausedDays}`. The clients enforce the limits: the server checks only the key and the value size, not the goal count or shape.

- `kind` is `daily_minutes` (`dailyMinutes` 1 to 600 per day) or `finish_quran` (`dailyMinutes` null). Days are `YYYY-MM-DD` in the device's zone. A window is at most 400 days.
- Target: `daily_minutes` is `dailyMinutes` times the days from `startDay` to `endDay`, both inclusive. `finish_quran` is 6,236 verse reads.
- Credit for a day is `max(ms / 60000, ayahsRead)`, so a verse read always counts at least one minute. Fast readers are never penalised.
- Progress sums the credited minutes (or verse reads) of days from `startDay` through the window end. The window end is `endDay` plus `pausedDays`, plus the days a live pause has run. It comes from server day totals, so reads on every device count.
- Pause sets `pausedFrom` to today. Paused days never use up the window: the end moves on by each paused day. Resume adds the paused days to `pausedDays` and clears `pausedFrom`, so the end is the same before and after. The target does not change.
- Streaks come from reads alone, not from goals: consecutive days with one or more verse reads, counting back from today, or from yesterday while today has none. A row is one register. There are no tombstones: clearing writes a null value with a newer stamp. The response has `preferences` in place of thread or bookmark rows. Consent flags and notes never sync.

## Pull

- `rev` is per user and per domain, taken inside the write transaction. Save `since` only after the page is durably applied.
- Delta (`since > 0`): changed rows by `rev`, tombstones included, 200 per page. Repeat while `more`.
- Full (`since = 0`): open threads, all progress, live folders and bookmarks. Keep `since = 0` and send `after = rev` while `more`. History comes from `/reading-session/v1/archive`.
- A tombstone is id, stamps, state and `rev` only. A bookmark tombstone keeps its key. Only a tombstone deletes: a missing row means nothing.
- Content stays 24 h for Undo, then the server scrubs it, and a scrubbed row ignores further writes.
- `since` below `tombstoneFloor`: full pull, then drop clean rows the pull did not name.
- `syncEpoch` changed, or `since` above `latestRev`: the server was restored. Full pull, and mark every local row dirty.

## The row is the queue

- `dirty` persists on the row, in the same write as the row. A round sends the current dirty rows, so 10,000 offline cursor moves are one row.
- Lifecycle writes persist at once. Cursor writes may debounce with a page-hide flush, so a hard kill can lose only the last moments of cursor movement.
- Dirty rows are never evicted and never dropped at sign-out, expiry or device revoke. They wait, per account, for that account to sign in.
- If local storage fails, keep the row in memory and surface the degraded status. Never report it synced.
- Across tabs, `dirty` is OR, except that an ack is final for the stamps it named.
- Clean rows of an account with no sign-in for 180 days are dropped on next launch.
- One tab polls while visible, elected with `navigator.locks`. Every tab polls if locks are missing. Native polls in the foreground and on resume. Nothing depends on background sync.

## Status table

| Response | Client behaviour |
| --- | --- |
| 401 | Sign-out state. Keep dirty rows for that account |
| 403 unverified, banned, or account mismatch | Pause. Never dead-letter. Keep dirty rows |
| 404 (auth nests unmounted) | Pause with backoff |
| 426 | Pause sync, show update prompt, keep reading |
| 429 | Hold this domain's pushes for `Retry-After`. Pulls continue (pull-only rounds) |
| 507 | A full pull is over the live quota (10,000 bookmarks or 200 folders). Surfaced as a failure with its message. The server never truncates, so nothing lands until the library shrinks. Rows stay dirty |
| 5xx, timeout, `SQLITE_BUSY`, malformed-request 400 | Retry with backoff and jitter. Rows stay dirty |

## Daily ceiling

Each account may take 5,000 accepted mutations per UTC day, counted across reading-session sync, bookmark sync and analytics events. A pull-only round (`mutations` empty) never counts. A row the server refuses as `rejected` never counts.

- A sync round with room for some rows applies them and returns `capped` for the rest. Those rows stay dirty and retry next day.
- A sync round with no room at all returns 429 with `Retry-After` to midnight UTC, and the client holds its pushes.
- An analytics batch is all-or-nothing: it returns 429 when it does not fit.
- The first refusal of a user's day writes one `rate_limit_events` row (`daily_ceiling`), never one per request.

## Versions

- Within a protocol, changes are additive: new fields and registers are optional. The server never removes a field or tightens a validator. A breaking change means a new protocol and a period serving both.
- Below the minimum protocol: 426 with no row applied. `tests/fixtures/sync-v1/` is frozen and must keep passing against every later build.
- Clients preserve unknown fields on rows and show an unknown enum value with a neutral default.
- A local store with a newer schema than the app stays untouched, and sync pauses.

## Edge cases

| Case | Required behaviour |
| --- | --- |
| Identical stamps from racing tabs or a clone | RFC 8785 byte tie-break, same result everywhere |
| Clock changed backwards mid-session | Stamps keep rising |
| Same-millisecond writes, counter overflow | Counter orders them; overflow bumps ms |
| Sleep, wake, or suspended device | Offset refreshes on next sync |
| Device clock hours behind | Edit after seeing a row beats that row. Before any sync, it may lose, and it converges |
| Device clock hours or a year ahead | Rewritten to server + 60 s, `last` reset, converges |
| Same thread moved on two devices | Highest cursor stamp wins; progress keeps the MAX |
| Same scope started on two devices offline | Two threads. Continue shows the newest |
| Thread ended on A, read further on stale B | Stays ended; B's cursor lands; B's reading keeps its progress |
| Deleted on A, read further on stale B | Stays deleted; B starts a new thread on its next read |
| Delete then Undo, with another device offline | Later stamp restores; other device converges |
| Folder renamed on A, reordered on B | Both survive |
| Folder deleted on A, bookmark added on B later | Bookmark moves to the default folder |
| One verse bookmarked on two devices offline in one folder | One row survives on every client |
| Same folder name created twice offline | Both kept |
| Device offline longer than 180 days | Full pull; nothing deleted revives; its own newer rows land |
| 10,000 offline scrolls | One dirty row |
| App killed after row persisted | Nothing lost |
| Hard kill during cursor debounce | Accepted limit: last moments of scroll position; lifecycle writes never lost |
| Response lost after server commit | Replay changes nothing |
| Duplicate or reordered requests | No effect |
| One invalid row in a batch | Only that row is refused |
| Multi-page pull interrupted | Resumes; `since` saved after durable apply |
| Local storage full or failing | Degraded status, never reported as synced |
| Server 5xx, deploy, `SQLITE_BUSY` | Backoff with jitter; rows stay dirty |
| Session expires with dirty rows | Rows wait; drain on sign-in to the same account |
| Signed-out reading, then sign-in | Folds silently into that account, per product decision, even on a shared phone |
| Account banned or email unverified during sync | Pause; rows kept; never dead-lettered |
| Same email re-registered | New user id. Accepted limit: the old partition's dirty rows never drain |
| Old app or stale tab on a new server | Works within a protocol; below minimum, 426 and reading continues |
| Unknown enum, field or register | Displayed neutrally and preserved |
| App downgrade with newer local schema | Store untouched; sync pauses |
| Position written on another client or layout | `surah:ayah` wins; page and ratio ignored unless same client class |
| Translation or snippet missing on this client | Display fallback; row value preserved |
| iOS or Android blocks background work | Sync on foreground and resume; nothing depends on background |
| Very large first sync or import above the daily ceiling | Stays dirty, continues next day, surfaced; pulls still work |
| Rate limited, many devices or tabs | One polling tab; `Retry-After` honoured |
| Two settings changed on two devices | Both survive |
| Second account signs in on one device | Separate partition, id, key and hint; nothing crosses |
| Remote sign-out while device is offline | Next call 401; dirty rows wait; after sign-in a new id via 409 |
| Signature formats differ by platform | Fixed 64-byte raw form on the wire; vectors enforce |
| Time-zone change or travel | Local day follows the user's zone; stamps are unaffected |
| Native app unused for weeks | 180-day session |
| Session rotation on a native client | `x-eq-session` replaced, CSRF refetched; lost response means sign in again |
| Browser tries to use `x-eq-session` | Refused; not in the CORS list |
| Stale native cookie jar | Impossible: native clients have no jar |
| New client implementation | Must pass all shared vectors and O32 |
| Many users behind one carrier IP | User bucket governs; IP bucket at least 10× |
