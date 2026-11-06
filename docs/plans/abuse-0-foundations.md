# Abuse phase 0: truth, measurement, ingress

[Back to index](../README.md)

#### A4b. Contact addresses

Needs MX proof first, so it waits for O3. Replace `corrections@easyquran.app` (`faq_script_answer`) and `salam@easyquran.app` (`legal_terms_contact_body`) in `web/messages/{en,ar}.json` with `privacy@`, `abuse@`, `legal@`, `support@` on `easyquran.fyi`. Add the `privacy@` address to the privacy page. Point `/contact` and `SITE.contactEmail` at `abuse@`/`support@`.

Done when: no `easyquran.app` address remains in messages; `/contact` no longer shows the owner's personal address.
Blocked by: O3

#### A7. Daily cross-domain ceiling, sync envelope, snapshot guard, subject_score seam

Build:

1. Daily ceiling: per-user in-memory TTL counter `writes:user:{id}`, 5,000 accepted mutations per day across the three route classes (session sync, analytics, bookmark sync). Exceed → 429 with `Retry-After` to midnight UTC. Write exactly one `rate_limit_events` row per user per day, on the transition only.
2. Envelope: S5's per-row `results` carry `rejected` and `capped`. Add summary counters from them. Verify existing `capped` occurrences in `bookmark_v1/controller.rs` and `reading_session_v1` first and reuse. Web client stores (`web/src/lib/bookmarks/*`, `web/src/lib/sync/*`) must surface `rejected` and `capped`, not treat them as soft skips.
3. Snapshot guard: when a user's snapshot would exceed the live quotas (10,000 bookmarks or 200 folders), return 507. Never truncate silently. After S6, pulls page instead of truncating, so `OPEN_SNAPSHOT_LIMIT` goes away and this guard covers only a full pull's total.
4. `subject_score` seam: optional parameter on the limiter, default 1.0, no consumer. A low score (future use) would lower the user budget floor (×0.5) and raise an operator event; it never blocks. Document in code. Typecheck only.

Files: `rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/db/sea_models/bookmark/*`, `rust/backend/api/src/modules/bookmark_v1/controller.rs`, `rust/backend/api/src/modules/reading_session_v1/controller.rs`, `web/src/lib/bookmarks/*`, `web/src/lib/sync/*`.

Done when: 5,001st mutation in a day → 429; exactly one `rate_limit_events` row per user per day; a capped account gets 507 on snapshot, never silent truncation; client store surfaces `rejected` and `capped` (web test); `subject_score` typechecks with default 1.0.
Blocked by: A5, S6
