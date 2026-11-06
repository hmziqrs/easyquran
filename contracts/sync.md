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
| Reading events | Insert-once by id | Idempotent |

Rival-key and folder-delete rules run on the set of rows after merging (`convergeBookmarks`). They keep the row's stamp. At equal stamps `deleted: true` has the higher canonical bytes, so a tombstone made on one replica beats the live copy everywhere.

## Edge cases

| Case | Required behaviour |
| --- | --- |
| Identical stamps from racing tabs or a clone | RFC 8785 byte tie-break, same result everywhere |
