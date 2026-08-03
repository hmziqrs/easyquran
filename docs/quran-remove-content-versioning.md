# EasyQuran — remove content versioning

> Status: **Plan — not yet implemented**
> (written 2026-08-03, against `feat/web-pwa-offline` @ `fb855f7`).
>
> Goal: delete `contentVersion` — the aggregate hash of the Quran databases
> that is used as a version number, an OPFS storage key, and an API/ETag
> component — everywhere. The databases are immutable; versioning them is
> machinery that manages a value that never changes.
>
> Companion docs to amend when this lands: [`quran-api.md`](./quran-api.md)
> (§8.1 defines the hash; envelope; `/version`; ETags), [`quran-web-delivery.md`](./quran-web-delivery.md)
> (§4 "Source invariants and contentVersion"; OPFS dir layout), [`quran-translations.md`](./quran-translations.md)
> (extends the same pattern to every translation pack), [`web-pwa-offline-plan.md`](./web-pwa-offline-plan.md)
> (§3.1, §4.4, §2.3-6).

---

## 1. Why

The Quran databases — Arabic today, every translation pack tomorrow — are
**immutable**. A given `quran-uthmani.sqlite` is correct forever; we never
patch a verse in place. Given that, `contentVersion` does no useful work: it is
a value computed from bytes that never change, threaded through the whole stack
to manage change-detection for a thing that does not change.

Worse, it is the exact anti-pattern we wanted to avoid: **the database's own
hash became its version number and its storage path.** `contentVersion` is
`blake3(uthmani ‖ simple-clean ‖ xml)` truncated to 16 hex chars
(`rust/backend/api/src/quran/loader.rs:96-103`), and the client stores each DB
under an OPFS directory named by that hash
(`web/src/lib/workers/opfs-cache.ts:40`). The plan in `quran-translations.md`
repeats the pattern for all 115 translation packs (`contentVersion =
sha256(pack)[..16]`, OPFS key `<id>-<contentVersion>/`). That is a versioning
system multiplied across every language for data that is frozen on arrival.

This also closes two open items the docs audit already moved toward:

- `easyquran-docs-audit-2026-07` decided "Arabic DBs immutable → drop the
  version segment from S3 keys." The **R2 keys are already version-free**
  (`web/src/lib/quran/view/source-profiles.ts:78,94`); this plan finishes the
  job by dropping the version from OPFS, the API, and the client.
- `easyquran-doc-compatibility` flagged "contentVersion not reproducible across
  builds" as a verified staleness bug. Deleting the concept deletes the bug.

**Integrity is not lost.** Per-file sha256 verification already exists, run by
code that is not going anywhere: the client verifies sha256+size on download
and on every cache hit, the SSG build validates the DB at build time, and the
Rust backend already computes and serves per-file `source_digests`. We keep all
of that. We remove only the *versioning* layer that sits on top.

---

## 2. What `contentVersion` is today (the footprint)

So this doc is self-contained, the full surface — every reference is listed in
the investigation that produced this plan. Condensed by layer:

**Rust backend (computes, asserts, serves, ETags):**
- `Cargo.toml:156-157` — `blake3 = "1.5"` dependency + its `content-version digest` comment. loader.rs is the **only** user → dependency is removed with it.
- `quran/loader.rs:96-103` — `blake3` aggregate → 16-hex `content_version`. The definition.
- `quran/loader.rs:95,105-115` — boot-time assertion vs `QURAN_CONTENT_VERSION` env; the `:95` comment ("reordering changes contentVersion…") is stale after removal.
- `quran/store.rs:275,296` — stored on `QuranStore`, exposed via `content_version()`.
- `quran_v1/controller.rs:676` — `/version` emits it (`VersionData` via `Envelope`); `:692-694` folds it into the `/version` ETag.
- `quran_v1/controller.rs:712` — `/health/ready` emits `content_version` on the `HealthReady` struct (`dto.rs:194-201`).
- `quran_v1/controller.rs:615-616` — **`/scripts`** also wraps `Envelope::new(ScriptsData{..}, &cv)`; the web client fetches `/scripts`, so this is a live, client-consumed field.
- `quran_v1/controller.rs:106,586,769,886` — the remaining `Envelope::new(.., store.content_version())` sites (surah/page/juz/random-ayah routes).
- `quran_v1/cache.rs:17,40-47` — **both** `weak_etag(content_version, key)` and `respond_cached(.., content_version, ..)` take it as a parameter; the `:70` doc-comment mentions it.
- `quran_v1/dto.rs:7-19` — the response `Envelope<T>` carries `content_version` on **every** payload.
- `quran_v1/dto.rs:176-179` — **`TranslationVersion`** struct: per-translation `content_version` carried in the `/version` `translations[]` array (the on-the-wire surface for translation versioning).
- `config/settings.rs:145-146,160` — `expected_content_version` field + env read (the `:145` comment is stale); `main.rs:444` — logs it.

**Wire / client decode:** `web/src/lib/quran/wire.ts:261-270` (`decodeVersionPayload`).

**Web client:**
- `web/src/lib/config/site.ts:44` — baked frozen constant `"32cc746d817cad9f"`.
- `web/src/lib/quran/manifest.ts:14,21,65,69` — `ResolvedManifest.contentVersion`; `/version` fetch with 3 s timeout; fallback to the baked constant.
- `web/src/lib/workers/opfs-cache.ts:27,40,48,56` — OPFS get/put **keyed by contentVersion**.
- `web/src/lib/workers/quran.worker.ts:105,121-137` — stores DB in that version dir; `pruneOldVersions()` deletes the others.

**Docs:** `quran-api.md` (~20 refs, §8.1), `quran-web-delivery.md` (§4),
`quran-translations.md` (`:316,360,430`), `web-pwa-offline-plan.md` (§3.1/§4.4),
plus passing mentions in `quran-normalization.md:917` and `quran-ssg-optimization-plan.md:351`.

Note `searchVersion` is a **separate** concept and is **not** in scope (§7).

---

## 3. Scope: remove vs keep

| Surface | Decision |
|---|---|
| `contentVersion` (aggregate blake3) | **Remove** — every layer. |
| OPFS/IDB storage keyed by `contentVersion` | **Remove the key axis** — store at a version-free path. |
| `pruneOldVersions()` | **Remove** — meaningless when there is one version; replaced by a one-time legacy migration (§5). |
| `QURAN_CONTENT_VERSION` env + boot assertion | **Migrate** to a per-file sha256 assertion (§6). |
| `Envelope.content_version` on every API response | **Remove** the field. |
| `/version` and `/health/ready` `content_version` field | **Remove.** `/version` keeps `search_version` + `source_digests`. |
| ETag `W/"<contentVersion>:<key>"` | **Simplify** to `W/"<key>"`; `/search` keeps folding `search_version`. |
| `quran-translations.md` per-pack `contentVersion = sha256[..16]` + `<id>-<contentVersion>/` OPFS key | **Remove** — translations are immutable too; stable key `translations/<id>/<id>.sqlite`, integrity via the catalog sha256 that already exists. |
| Per-file sha256 verification (download, OPFS hit, SSG build) | **Keep — unchanged.** This is integrity, not versioning. |
| Rust `source_digests` (per-file sha256 served on `/version`, `/health/ready`) | **Keep — promoted** to the sole integrity surface. |
| R2 object keys | **Already version-free** — no change. |
| `searchVersion` | **Keep** (§7). |

---

## 4. Target end-state, by layer

### 4.1 Rust backend

- **`Cargo.toml`** — remove the `blake3 = "1.5"` dependency (and its comment); loader.rs is its only user. `Cargo.lock` updates automatically.
- **`loader.rs`** — delete the `blake3` block (`:96-103`); delete `content_version` from the returned `QuranStore`; delete the stale `:95` contract comment. Keep the per-file `file_sha256` computation (`:121,126,146`) — that feeds `source_digests`, which stays.
- **`store.rs`** — drop the `content_version: Arc<str>` field and `content_version()` accessor.
- **`settings.rs` / `loader.rs` boot assertion** — replace `QURAN_CONTENT_VERSION` with two pinned per-file sha256 values (uthmani + simple-clean), asserted against the computed `file_sha256` at boot; remove the stale `:145` comment. These are the same digests already pinned in `web/src/lib/quran/view/source-profiles.ts:80,96`; mirror them in Rust config so both sides assert the same bytes. (§6.)
- **`dto.rs`** — remove `content_version` from `Envelope<T>` and drop the second argument of `Envelope::new`; remove the `content_version` field from `HealthReady` (`:196`) and `TranslationVersion` (`:178`). `VersionData` itself has no such field (it rides on the `Envelope`), so it is untouched.
- **`controller.rs` `/version` (`:676`)** — emit `{ api_version, search_version, source_digests, translations }`; drop `content_version`. ETag becomes `weak_etag(quran::SEARCH_VERSION, "version")`. `TranslationVersion` entries in the `translations[]` array keep only `id` (+ any non-version fields).
- **`controller.rs` `/scripts` (`:615-616`)** — `Envelope::new(ScriptsData{..})` drops the version arg; `respond_cached` call drops the `content_version` arg.
- **`controller.rs` `/health/ready` (`:712`)** — drop `content_version` from `HealthReady`, keep `search_version` + `source_digests`.
- **`controller.rs` remaining sites (`:106,586,769,886` + `:691` comment)** — all `Envelope::new(..)` / `respond_cached(..)` calls lose the version arg; reword the `:691` comment to reference `search_version` only.
- **`cache.rs`** — `weak_etag(canonical_key)` for content routes (key-only is a valid ETag for immutable data); `/search` folds `search_version` as today. Both `respond_cached` and `respond_cached_with_etag` (`:40,71`) lose their `content_version` parameter; reword the `:70` comment.
- **`main.rs:444`** — log `source_digests` instead of `content_version`.
- **OpenAPI:** the schemas are utoipa-derived from the DTO structs, so removing the fields auto-updates the served spec — no separate spec file exists or needs editing.

### 4.2 Wire / API contract

- `web/src/lib/quran/wire.ts` — remove `contentVersion` from the decoded version payload; keep `searchVersion`. `decodeVersionPayload` shrinks accordingly.
- The response envelope loses `content_version`; any decoder that currently ignores it needs no change, but the type/test is updated.

### 4.3 Web client

- `web/src/lib/config/site.ts:44` — delete the `contentVersion` constant from `QURAN`.
- `web/src/lib/quran/manifest.ts` — remove `contentVersion` from `ResolvedManifest`; `resolveManifest` no longer reads it from `/version` (it still reads `scripts`, and `searchVersion` is retained for the worker's search path). `baked` drops the field.
- `web/src/lib/workers/quran.worker.ts:105` — `ensureArtifact(spec)` loses the `contentVersion` argument.

### 4.4 OPFS / IDB storage (the "version became the path" fix)

This is an **interface-level** change, not a call-site tweak. `version` is the
*first* parameter of the `ByteStore` interface today.

- `web/src/lib/workers/storage.ts` — reshape `ByteStore` from `get(version, key)` / `put(version, key, bytes)` to `get(key)` / `put(key, bytes)`. Concretely: `createOpfsStore` drops the `versionDir(version)` directory layer (store files directly under the root dir, e.g. `easyquran/<id>.sqlite`); `createIdbStore` drops the `${version}:${key}` prefix (key by `<id>` only). `pruneOpfs(rootDir, keepVersion)` and `pruneIdb(dbName, storeName, keepVersion)` are removed — repurposed as the one-time legacy migration in §5, then deleted.
- `web/src/lib/workers/cached.ts` — drop the `version?` option from `ensureCached`'s opts (the `store && version && key` gates collapse to `store && key`).
- `web/src/lib/workers/opfs-cache.ts` — `ensureArtifact(spec)` no longer takes a version; `opfs.get/put` and `ensureCached` calls shed the version arg. sha256 verification on hit (`:43`) and on download (`download.ts:30`) is unchanged and remains the integrity gate.
- `web/src/lib/workers/quran.worker.ts:121-137` — delete `pruneOldVersions()` and its call; with one version there is nothing to prune after the one-time migration (§5).

### 4.5 Translations (`quran-translations.md`)

Apply the identical treatment so the user's "English, Urdu, any language" point
is resolved by construction, not just for Arabic:

- Drop per-pack `contentVersion = sha256(pack)[..16]`.
- OPFS pack path becomes version-free: `translations/<id>/<id>.sqlite`.
- Integrity = the sha256 + size the catalog already carries; the client verifies on download exactly as it does for Arabic. No version comparison, no re-download-on-version-bump logic.

### 4.6 Docs to amend

- `quran-api.md` — remove §8.1's hash definition, the `contentVersion` column of the version table, every envelope/ETag/`/version` mention; state that Arabic integrity is per-file `source_digests`.
- `quran-web-delivery.md` — rewrite §4: immutable DBs, version-free OPFS path, integrity via sha256.
- `quran-translations.md` — remove `:316,360,430` version machinery; stable pack keys.
- `web-pwa-offline-plan.md` — strike `contentVersion` from §3.1's version table and §4.4's OPFS cleanup (it becomes a one-time legacy migration, §5 below).
- `quran-normalization.md:917`, `quran-ssg-optimization-plan.md:351` — drop the passing mentions.

---

## 5. Migration for existing clients

Today's installed clients have DBs stored under version-keyed paths
(`easyquran/<contentVersion>/<id>.sqlite`, IDB keys prefixed `contentVersion:`).
After this change the worker looks up a version-free path, misses, and
**re-downloads once** (sha256+size verified, ~2.4 MB Arabic), then stores at the
new path. From the next launch it is a permanent cache hit.

To avoid leaving the old version-keyed entries as permanent orphans:

- Replace `pruneOldVersions()` with a one-time `migrateLegacyVersionedStorage()`
  that runs on first boot of the new build: enumerate OPFS subdirectories / IDB
  keys matching the legacy `<hex-version>` shape, delete them, set a flag in
  `sessionStorage` (or equivalent) so it never runs again. Best-effort, logged
  on failure — a stranded legacy copy costs storage, never correctness.

This is the only behavior change a user sees: one re-download of the Arabic DBs
on the first launch after the deploy that ships this. Translations, when they
land, have no installed base yet, so no migration.

---

## 6. Integrity after removal (what keeps us safe)

Per-file sha256 — already implemented, retained unchanged:

- **Client download:** `download.ts:30` rejects on sha256 mismatch before the bytes are ever used.
- **Client cache hit:** `opfs-cache.ts:43` re-verifies on every OPFS read, so a truncated/swapped cached file is caught.
- **SSG build:** `quran-sqlite.ts:118` validates the DB against the registered digest at build time; `resolveSourceProfile` throws on mismatch.
- **Rust boot:** today the only runtime check is the aggregate `QURAN_CONTENT_VERSION` assertion. Migrate it to assert the two computed per-file sha256s against pinned expected values (the same values `source-profiles.ts:80,96` already pins for the web). Safety preserved; the "version" is gone.

Lighter alternative, if even the boot assertion is deemed unneeded: compute and
**log** the per-file sha256s at boot without asserting. The client and SSG paths
already enforce integrity independently. Recommended path is to keep the
assertion (§4.1) — it catches "wrong DB baked into the image" at boot, not in
production.

---

## 7. What we keep, and why

- **`searchVersion`** — *not* database versioning. It is a semantic tag
  (`arabic-search-v2`) for the **search normalization rules**, and it has
  already moved (v1 → v2, the U+0670 drop). It genuinely versions behavior that
  changes, drives the in-worker corpus rebuild and the `/search` ETag. It stays.
- **Per-file sha256 + `source_digests`** — integrity, not versioning. Stays and
  becomes the sole integrity surface.
- **R2 stable keys** — already version-free; no work.

---

## 8. Rollout phases

Each phase ships independently. Phases 1–3 can be one PR if preferred; they are
separated only to make review and rollback boundaries clear.

**Phase 1 — Rust backend.** Drop the blake3 `content_version` from
`loader`/`store`; migrate the boot assertion to per-file sha256; remove
`content_version` from `Envelope`, `/version`, `/health/ready`; simplify ETags.
*Accept:* backend boots and asserts per-file sha256; `/version` and `/scripts`
return no `content_version`; `cargo build` is warning-free with `blake3` gone.
Tests updated: `loader.rs:538-540` (the `content_version().len() == 16` assertion
is deleted), `loader.rs:507,646` + `quran_v1.rs:28` (`expected_content_version:
None` fixtures leave the `Settings` struct as the field is removed),
`quran_v1.rs:224` (`assert!(body.get("contentVersion").is_some())` → removed or
flipped to `.is_none()`); all lib + `quran_v1` integration tests pass.

**Phase 2 — web client + storage.** Remove `contentVersion` from `site.ts`,
`manifest.ts`, `wire.ts`; flatten OPFS/IDB to version-free keys; replace
`pruneOldVersions` with the one-time legacy migration. *Accept:* fresh client
downloads, sha256-verifies, caches at the version-free path; a second launch is
a cache hit; a client upgraded from the old build re-downloads once and the
legacy version-keyed entries are cleaned up; `svelte-check` + `tsc` + vitest
clean; the SW precache/offline behavior from `web-pwa-offline-plan.md` is
unaffected (it never keyed on `contentVersion`). Tests updated:
`wire.test.ts:212-218` (`decodeVersionPayload` cases drop the `contentVersion`
expectations), `worker-client.test.ts:46` (mock `ResolvedManifest` drops the
`contentVersion: "v1"` field).

**Phase 3 — translations doc + future pack handling.** Amend
`quran-translations.md` so the pack design uses stable keys + catalog sha256,
no per-pack `contentVersion`. *Accept:* the translation build/storage design
matches the Arabic treatment; no version machinery to carry into the
translations implementation.

**Phase 4 — docs sweep.** Amend `quran-api.md`, `quran-web-delivery.md`,
`web-pwa-offline-plan.md`, and the two passing-mention docs per §4.6. *Accept:*
`rg content[_-]?[Vv]ersion` across `docs/` returns only this plan and
historical decision-log lines that explicitly reference the removal.

---

## 9. Decision log

- **(a) Delete, not freeze.** A frozen constant still costs the OPFS version
  axis, the `/version` round-trip, the ETag plumbing, and the mental overhead.
  If the value never changes, the concept should not exist.
- **(b) Keep per-file sha256 as integrity.** Removing versioning must not weaken
  correctness. sha256 verification already exists at download, cache-hit, and
  build; it stays and becomes the canonical integrity surface.
- **(c) Migrate, don't drop, the boot assertion.** A boot-time check that the
  right bytes are loaded is independent of "will we ever update." Pin per-file
  sha256 and assert against it.
- **(d) `searchVersion` is out of scope.** It versions normalization semantics
  that have actually changed; conflating it with database content versioning
  would reintroduce the confusion this plan exists to remove.
- **(e) Translations treated identically.** The user's point is not Arabic-only:
  every language pack is immutable. The removal applies to the translation pack
  design now, before it is built, so the versioning pattern is never multiplied.

---

## 10. Open questions

1. **Boot assertion pinning in Rust:** mirror the web's pinned sha256s into Rust
   config (two values), or read them from a shared build-emitted artifact?
   Default: mirror — they change only when a DB is corrected, which is the
   ~never case this whole plan is predicated on.
2. **One-time migration flag storage:** `sessionStorage` is per-tab; a
   `localStorage` flag or an IDB marker survives across the migration launch.
   Default: a small IDB marker so cleanup runs exactly once per client.
3. **`Envelope` field removal vs a deprecation cycle:** the public API is
   currently unused in production (compose has the API commented out) and is
   SSG-consumed, so a clean removal is safe. Default: remove cleanly, no cycle.

---

## 11. Audit notes (verified non-issues + out-of-scope)

Surfaces checked during the completeness audit that are **not** part of this
removal — recorded so the implementer doesn't re-investigate:

- **No `content_version` SQLite column.** `quran-api.md:449`
  (`content_version TEXT NOT NULL`) is doc-only; no such column exists in `db/`
  or in any Rust schema. No hidden database surface to touch.
- **No committed OpenAPI spec file.** Schemas are utoipa-derived from the DTO
  structs (`dto.rs`, `controller.rs`, `docs.rs`, `router.rs`); removing fields
  auto-updates the served document. There is no `openapi.json`/`swagger.json` to
  edit by hand.
- **`QURAN_CONTENT_VERSION` is code + docs only.** It is absent from
  `.env.example`, both Dockerfiles, `docker-compose.yml`, and `deploy.sh` — no
  deploy-config cleanup, only `settings.rs`.
- **The web client consumes `content_version` only via `/version`**
  (`manifest.ts`). No other wire decoder reads the envelope field, and the SSG
  server code (`web/src/lib/server/`) never touches it. Shrinking the envelope
  is therefore safe end-to-end.
- **Service worker, offline pack, and build scripts are clean.**
  `service-worker.ts`, `web/src/lib/offline/`, `web/scripts/` (incl.
  `gen-offline-pack.ts`), and `quran-meta/` contain zero `contentVersion`
  references — the PWA layer is fully decoupled and unaffected.

**Out of scope, pre-existing — flag only:** `searchVersion` has a version skew
in the tree right now — Rust pins `arabic-search-v1` (`quran/store.rs:8`) while
the web search fixtures say `arabic-search-v2`
(`web/src/lib/quran/search/__fixtures__/queries.json:2`). This predates and is
unrelated to this removal; do not "fix" it here, and don't mistake it for a
regression caused by this work. Reconciling `searchVersion` is its own task.
