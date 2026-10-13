# Reading model — sessions, bookmarks, reading insights

Status: **in progress** (2026-10-14). Supersedes next-features Features 2, 3, 4, 6 and §9's
bookmark/session parts. Breaking change by design: nothing here is deployed, so the
`m000012`–`m000014` migrations are rewritten in place and no legacy client shape is kept.

Goal: **zero friction**. The reader never asks the user to manage anything — threads appear,
resume and retire on their own; bookmarks save in one tap; insights collect themselves.

## 1. Layers

| Layer    | Job                     | Lives                                                                           |
| -------- | ----------------------- | ------------------------------------------------------------------------------- |
| Scope    | WHAT is read            | static: `surah:N`, `page:N`, `juz:N`, `hizb:N`, `snippet:slug`, `range:S:A-S:A` |
| Session  | ONE continuous thread   | synced (`reading_session_v1`), local per owner                                  |
| Card     | WHEN to read (widget)   | local (`easyquran.home.cards`), opens a session                                 |
| Event    | WHAT got read           | append-only (`user_analytics_v1`), per-owner queue                              |
| Bookmark | SAVED places in folders | synced (`bookmark_v1`), local per owner                                         |

Snippets (`#lib/data/snippets.ts`) are the slice catalogue — references only, Quran DBs stay
immutable. Prayer-time anchoring is out of scope (moves to easyislam.dev later); cards keep
fixed `HH:MM` slots.

## 2. Reading sessions

**A session is one continuous thread of reading.** Continuing (next/prev ayah, page, surah,
juz, scrolling, reload) stays in the thread. Entering from somewhere else starts or resumes
a thread. A thread is device-independent: opening it on another device continues the same id.

### 2.1 Boundary rules

Entry points tag the navigation (`data-session-origin` on links, `markEntry()` before a
programmatic `goto`). On every reader navigation the sessions store resolves:

1. **Explicit resume** (Continue card / handoff, `origin=resume` + id) → that thread.
2. **Entry with an exact ayah** (search hit, palette go-to-verse, bookmark, deep link with an
   ayah, card pointing at an ayah/range) → **new thread** at that ayah.
3. **Entry to a whole scope** (surah list, sidebar, juz/page index, deep link without ayah) →
   if an open thread's cursor sits in that surah/scope, **resume it** and show a
   **"Start over"** pill; else new thread.
4. **Cards and snippets** → always a new thread per open (each recitation is its own run).
5. **No tag, coming from a reader route** → continuation (keeps the tab's thread).
6. **No tag, first load / reload** → continuation when the tab's current thread (sessionStorage)
   covers the target; else rule 2/3 with `origin=link`.

Origins: `index | search | bookmark | card | snippet | link | resume`. The tab's current
thread id lives in `sessionStorage` (one tab = one thread at a time; reload keeps it).

### 2.2 Lifecycle

- **open** — `ended_at IS NULL` and touched within 30 days. Shown in Continue.
- **ended** — `ended_at` set with `end_reason`:
  - `completed` — cursor reached the end of a bounded scope (snippet, range, card range);
  - `restarted` — user tapped "Start over" (a fresh thread replaces it);
  - `dismissed` — user removed it from Continue.
- **stale** — open but untouched > 30 days. Derived at query time, never stored.

Ended + stale threads are the **archive**: never deleted, not in Continue, readable through
`GET /reading-session/v1/archive`. A user delete from history is a tombstone.

### 2.3 Data (`m000013`, app SQLite)

```
reading_sessions(user_id, id, origin, origin_ref?, scope, bound_end?, surah, ayah,
                 local_page?, ratio?, source_id?, device, started_at, updated_at,
                 ended_at?, end_reason?, deleted_at?)   PK (user_id, id)
reading_progress(user_id, surah, furthest_ayah, updated_at)   PK (user_id, surah)
```

`bound_end` = `"S:A"` for bounded scopes. `device` = `phone | tablet | desktop` (handoff copy
"on phone"). Progress is account-level, merged with MAX (never regresses).

### 2.4 API

- `POST /reading-session/v1/sync` `{expectedUserId, mutations[]}` — `session.upsert` (whole
  row, LWW on `updatedAt`), `session.delete` (tombstone), `progress.upsert` (MAX). Returns
  `{applied, skipped, sessions, progress}` with `sessions` = open threads, newest 20.
- `GET /reading-session/v1/archive?before=<RFC3339>&limit=20` — ended or stale threads,
  newest first, `{sessions, next}`.

### 2.5 Surfaces

- `/yours` + home **Continue**: open threads (max 5), newest first — surah, `ayah/total`,
  "on phone · 5 min ago" when another device touched it last. Tap = resume. ✕ = dismiss.
- History list on `/yours` (archive, load more).
- Reader: "Start over" pill after an auto-resume.
- Palette / hotkey "Continue reading" = newest open thread.
- Removed: named sessions, kinds, switcher, archive toggles, settings section.

## 3. Bookmarks

- **Every bookmark lives in a folder.** Each owner has a **default folder** ("Bookmarks") with a
  deterministic id `UUIDv5(NS, "user:{id}")` (anon: `UUIDv5(NS, "local")`), so offline devices
  converge without duplicates. It can be renamed, never deleted; the server ensures it exists
  each round.
- **A verse may sit in many folders.** Live key `(user, folder, surah, ayah, end_surah, end_ayah)`.
- Reader tap: not saved → save to the **last-used folder** (default at first) + toast
  "Saved to Bookmarks · Change". Saved → remove from every folder + Undo toast.
  "Save to…" lists folders as checkboxes.
- **Delete folder** = AlertDialog "Delete 'Hifz' and its 23 bookmarks?" → tombstones folder
  and its bookmarks; Undo toast restores both.
- **Signed-out users get full folders**, stored locally. On sign-in local data merges into the
  account silently (local default → account default; other folders as-is) with a toast. No
  review dialog.
- Dropped: `session_id` on bookmarks, folder `kind`.

Data (`m000012` rebuild): `bookmark_folders(user_id, id, name, is_default, position?, created_at,
updated_at, deleted_at?)`, `bookmarks(id, user_id, folder_id NOT NULL, source_id?, surah, ayah,
end_surah, end_ayah, created_at, updated_at, deleted_at?)`.

## 4. Reading insights (analytics)

**Always on, personal only** (owner decision 2026-10-14; privacy copy updated). No consent
switch. Global aggregates are out of scope.

### 4.1 What counts as read — hafiz-aware

A DOM tracker observes every `[data-verse-anchor]` (all reader modes) with an
IntersectionObserver:

- A verse is **read** when it leaves the viewport upward, or the reader navigates / hides
  while it is visible, having been on screen for at least
  `max(150 ms, letters × 10 ms)` (≈ 25 words/s — far beyond the fastest hadr recitation, so a
  hafiz reading fast always counts; only flings and jumps fall below it).
- Leaving at the bottom (scrolling back) does not count. The same verse counts at most once
  per 30 s in a thread (edge jitter), so real repetition still counts.
- **Time**: active wall time (tab visible, last interaction ≤ 5 min ago — long enough for a
  slow page recitation without scrolling) split across the verses read in that window by
  letter count.

### 4.2 Pipeline

Event `{id, sessionId, surah, ayah, sourceId, ms, localDay, at}` → per-owner durable queue →
`POST /user/analytics/v1/events` (batch ≤ 100, idempotent by `id`). Server tables (`m000014`):
`reading_events` (raw, 90-day retention), `reading_daily` (day × surah), `reading_ayah_totals`
(surah × ayah: reads, ms) — the retention-proof source for "most read". Anonymous events roll
up locally and their queue uploads on sign-in.

## 5. Build order

1. Backend: migrations, models, modules, tests.
2. Web sessions + entry tagging + read tracker + analytics client.
3. Web bookmarks: folders, anon local store, silent merge, reader save/picker.
4. UI: Continue/history, bookmarks page, settings cleanup, shadcn time picker in the card
   schedule editor.
5. Gates green (`bun run check|lint|test`, `cargo test`, clippy), docs trimmed.
