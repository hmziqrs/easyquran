# EasyQuran — architecture

One source of truth: data, API, delivery, caching. Decisions + contracts + open work only. Implemented detail lives in code; this doc does not record what is already built.

## Hard rules

- Quran databases (Arabic + every translation) are **immutable**. Sourced from Tanzil.net; integrity fixed at ingest, never re-checked at runtime. No modifications, no versioning.
- **No content versioning.** No `contentVersion`/`searchVersion`, no version segment in R2 keys, no hash-keyed cache dirs. A database's identity is its **id** (`quran-uthmani`, `en.hilali`, …).
- **No runtime hash verification.** Databases are trusted (immutable + immutable R2 keys). Do not reintroduce sha256/blake3 keying or digest-pinning at runtime.
- Render = **SSR + disk-persisted TTL cache** for translated-page HTML responses. Not ISR. (Databases themselves are cached in memory — see Caching.)

## Data sources

- `quran-uthmani.sqlite` = display text. `quran-simple-clean.sqlite` = search corpus (undiacriticated, regular alef). Both: `quran_text("index" PK, sura, aya, text)` + `idx(sura,aya)`, 6236 rows, used **directly read-only**. Not consolidated; no canonical db is built.
- `quran-data.xml` = metadata only (no verse text): 114 surah, 6236 ayah, 604 pages, 30 juz, 556 ruku, 240 hizb-quarter, 7 manzil, 15 sajda. Web consumes one compact `web/static/quran-meta/quran-data.json`; Rust parses the XML in memory at boot.
- **No FTS.** Search = normalize + substring-scan the 6236 simple-clean rows.
- **Bismillah:** surahs 95 & 97 carry a shadda variant (`بِّسْمِ`). Strip via a diacritic-insensitive prefix match, not exact.
- Translations: 115 Tanzil dumps → one `<id>.sqlite` each (same `quran_text` schema). **Non-commercial license** — revisit if the project monetizes.

## Caching — where each thing lives

Databases are immutable + read-only: no write handling anywhere.

- **Backend (Rust) — databases in memory:**
  - Arabic DBs loaded at boot, kept resident for fast access.
  - Translation DBs held in an in-memory pool; TTL/LRU decides **how many** stay and **how long**, by popularity + hit-rate. Disused → evicted from memory.
- **Backend — translated-page SSR HTML on disk:** on-demand render cached to disk with TTL. (Arabic is SSG, not SSR — not cached here.)
- **Backend — API JSON: not cached.** Served fresh each request straight from the in-memory databases (fast because they are resident).
- **Web client — OPFS:** Arabic eager on boot; translations lazy. Adaptive-TTL (use renews, disuse expires) as the worst-case backstop for a tester loading ~100 databases. A normal user loads 1–2 → nothing evicts. Reuse the Arabic save/load/download/read-chunk machinery for translations — no new chunking code. **Key = DB id.**

## API contract — Rust `/quran`

Module `quran_v1`, routes nest at `/quran`. **No `/quran/v1`. No `/version`.**

- `GET /quran/scripts` — advertise the immutable R2 files (id, sizeBytes, downloadUrl).
- `GET /quran/health/ready` — liveness.
- `GET /quran/search?q=…` — substring search; `kind` discriminator (`ayah` | `opener`).
- `GET /quran/sources/{id}/surah/{n}` → `Envelope<QuranSurahTextDto>`.
- `GET /quran/sources/{id}/range?from=&to=` → `Envelope<RangeText>` (cap 300).
- **Auth:** web OAuth code flow (google/apple/facebook/github) + mobile `POST /auth/{google,apple,facebook}/v1/token` (client SDK token → cookie session, **not** JWT; GitHub is web-only, no token flow). The three `/token` routes are CSRF-exempt.
- `Envelope<T> = { data: T }`. Closed error shape (400 validation / 404 / 429 / 5xx) — a failed read never becomes an empty surah.
- **Translation API (open work):** granular verse/range JSON, served fresh from the in-memory translation pool (no JSON caching). Serves cold client reads before the full sqlite is cached in OPFS.

## Normalization + canonical view — parity contract

One rule set, two implementations (Rust + web Worker) that must return identical ordered results:

- **normalize:** drop U+0670 (superscript alef), fold U+0671→alef, strip U+0640 (tatweel). Then substring-scan normalized simple-clean.
- **canonical view** splits each surah into **body + opener** units (opener = rank 0, ayah = rank 1); offsets are **Unicode scalar** (Rust `char` = web `Array.from`).
- **bismillah classification:** `FirstAyah | EmbeddedPrefix | None`.
- A shared fixture suite enforces parity. A rule change ships new Rust + web together; it never mutates a sqlite.

## Web delivery

- **Arabic = SSG.** Build-time `node:sqlite` reads uthmani and prerenders reader pages — real HTML for SEO and first paint. Hydration adopts the prerendered page; no WASM on the critical path.
- **Translated pages = SSR + disk-TTL** (on-demand render, cached to disk, TTL). Needs `adapter-node` (migration is open work). The Arabic core stays prerendered.
- **Offline (OPFS):** Arabic eager on boot; translations lazy. Reads + search run local once cached; adaptive-TTL evicts the disused. The service worker precaches the shell + immutable assets only and passes `/quran/**` through.
- **Service worker constraint:** SvelteKit forbids `$lib`/relative/npm imports in the SW, so the SW↔client contract is **duplicated** (SW-inline + `web/src/lib/offline/*`). Audit both sides on any contract change. Updates are an atomic SW-lifecycle cache swap — install verifies the complete new cache before activate; one accepted update reloads all open tabs.

## Open work

- Translation delivery end-to-end: API (granular, disk-TTL + popularity) + R2 full-sqlite (OPFS adaptive-TTL, reusing Arabic machinery). Not wired yet.
- `adapter-static → adapter-node` + the disk-TTL SSR render cache for translated pages.
- Per-id OPFS pruner that TTL-evicts disused databases (the backstop above).
- Mobile (Flutter) parity.

## Appendix

Four genuinely-empty source verses — immutable source data, **not** bugs; do not "fix": `fa.safavi 80:39`, `ku.asan 108:3`, `sq.mehdiu 21:56`, `sq.mehdiu 77:14`. Full detail in [`docs/research/translation-empty-verses.md`](./research/translation-empty-verses.md).
