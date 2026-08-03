# EasyQuran — remove content versioning

> Status: **Shipped on `master`** — removal landed via PR #7 (PWA/offline) and
> PR #8 (Rust API). Verified 2026-08-04 against `8982220`.
>
> `contentVersion` — the aggregate hash of the Quran databases, formerly used as
> a version number, an OPFS/IDB storage key, and an API/ETag component — is gone
> from every layer. The databases are immutable; versioning them was machinery
> that managed a value that never changes.
>
> This document was originally a forward plan (written 2026-08-03 against
> `feat/web-pwa-offline @ fb855f7`). The implementation overtook it, so it has
> been recast as a **verification + decision record**: what was removed, where it
> landed, the two factual corrections to the original plan, and the three edge
> cases the implementation resolved.

---

## 1. Why

The Quran databases — Arabic today, every translation pack tomorrow — are
**immutable**. A given `quran-uthmani.sqlite` is correct forever; we never
patch a verse in place. Given that, `contentVersion` did no useful work: it was
a value computed from bytes that never change, threaded through the whole stack
to manage change-detection for a thing that does not change.

Worse, it was the exact anti-pattern we wanted to avoid: **the database's own
hash became its version number and its storage path.** That is a versioning
system multiplied across every language for data that is frozen on arrival.

**Integrity is not lost.** Per-file sha256 verification — at download, on every
cache hit, at SSG build, and as a pinned boot assertion — is the sole integrity
surface and is fully intact (§5).

---

## 2. What `contentVersion` was, and what happened to each surface

Verified against `master @ 8982220`. Every former reference is gone from
`rust/backend/api/src/**` and `web/src/**` (`git grep 'content[_-]?[Vv]ersion'`
returns nothing in source).

### 2.1 Rust backend — removed

- **`Envelope<T>`** is now `{ data: T }`; `Envelope::new(data)` is single-arg
  (`dto.rs:7-15`). The `content_version` field that rode on every payload is gone.
- **ETags** are built by `weak_etag(tag, canonical_key)` (`cache.rs:16`); every
  content route folds `store.etag_tag()` = `source_digests.uthmani`
  (`store.rs:290-292`, called at `controller.rs:125,633,758,874`). The
  `content_version` parameter is gone from `respond_cached` / `respond_cached_with_etag`.
- **Boot assertion** migrated from the `QURAN_CONTENT_VERSION` env check to pinned
  per-file sha256: `loader.rs:16-17` (`GOLDEN_UTHMANI` / `GOLDEN_SIMPLE_CLEAN`),
  asserted at `loader.rs:75-87`, mirrored in tests at `loader.rs:496-497`.
- **`/health/ready`** emits `source_digests` + counts, no `content_version`
  (`controller.rs:707-709`, `dto.rs:188-196`).
- **`main.rs:445`** logs `source_digest` instead of `content_version`.
- **`settings.rs`** has no `expected_content_version` field.
- **Dead dependency removed:** `blake3 = "1.5"` (`Cargo.toml:156-157`) had zero
  callers in `src/` once the aggregate hash was deleted; dropped in this closeout.
  `cargo check --all-targets` is warning-free.

### 2.2 Wire + web client — removed

- **No `/version` endpoint exists.** The route table (`quran_v1/mod.rs:13-40`)
  has no `/version`; the web client fetches **`/scripts`** (`manifest.ts:43`),
  decoded by `decodeScriptsPayload` (`wire.ts:248`). The former
  `decodeVersionPayload` / `VersionPayload` are gone.
- **`ResolvedManifest`** is `{ scripts, source }` only — no `contentVersion`
  (`manifest.ts:13-16`).
- **`site.ts`** no longer carries the baked constant `"32cc746d817cad9f"`.
- **Tests:** `tests/quran_v1.rs:223` asserts `body.get("contentVersion").is_none()`
  — the negative assertion the original plan anticipated.

### 2.3 OPFS / IDB storage — the "hash became the path" fix

The version axis was removed **as a mutable version**, but the storage interface
kept a two-axis shape, repurposed to carry integrity:

- `ByteStore` is `get(tag, key)` / `put(tag, key, bytes)` (`storage.ts:2,16,64`),
  where `tag` is the per-file **`spec.sha256`** (`opfs-cache.ts:39,47,55`).
- OPFS path: `easyquran/<sha256>/<id>.sqlite`; IDB key: `<sha256>:<id>`.
- `pruneOldVersions()` / `pruneOpfs` / `pruneIdb` / `versionDir` are all gone.

This is accepted, not a TODO — see decision (B) in §4.

### 2.4 Translations — the pattern was never multiplied

`quran-translations.md` already uses full-digest identity (`<id>-<sha256>/`,
`:433`; `tanzil/translations/sqlite/<id>/<sha256>/<id>.sqlite`, `:363`). No
per-pack `contentVersion = sha256[..16]` was ever built.

---

## 3. Two corrections to the original plan

The forward plan contained two factual errors, recorded here so they don't
re-emerge:

1. **The hash definition.** The plan's §1 defined `contentVersion` as
   `blake3(uthmani ‖ simple-clean ‖ xml)[..16]`. It was almost certainly
   `sha256(uthmani)[..16]`. Smoking gun: `GOLDEN_UTHMANI` (`loader.rs:16`) is
   `32cc746d817cad9f...`, whose first 16 hex chars are exactly the deleted web
   constant `32cc746d817cad9f`. This is also why `blake3` became a dead
   dependency (§2.1) — `sha2`, already present, computed the real digest.

2. **The digest conflation.** The plan's §6 claimed the boot assertion pins "the
   same values `source-profiles.ts` pins." It does not — they are different
   digests over different inputs:
   - **`source_digests`** is a *corpus* digest (sha256 of the normalized verse
     text), `loader.rs:73-74`, asserted at boot via `GOLDEN_UTHMANI`/`GOLDEN_SIMPLE_CLEAN`.
   - **`file_sha256`** is the sha256 of the `.sqlite` *file* (`loader.rs:122`),
     served per-artifact on `/scripts` (`controller.rs:654` → `dto.rs:169`), and
     pinned in `source-profiles.ts:80,96` (`581cc540…` / `a0c52760…`).
   Both are kept; they are simply not the same value.

---

## 4. Resolved edge cases (three owner decisions, 2026-08-04)

**(A) `searchVersion` — removed, not kept.**
The original plan's §7 said to *keep* `searchVersion` (a semantic tag for search
normalization). It was not kept: it is absent from all code under every name
(`searchVersion` / `search_version` / `arabic-search-v*`), surviving only in
docs. The `/search` ETag today keys on `store.etag_tag()` (uthmani digest) +
`sha256(query)[..8]` + limit/offset/script (`controller.rs:872-876`); the
in-worker corpus rebuilds once per worker lifetime (`quran.worker.ts:166`).
**Decision: accept the removal.** The §11 "Rust `arabic-search-v1` vs web
`arabic-search-v2` skew" note was phantom — neither constant ever existed on
`master`.

> Caveat for the future: because normalization rules are baked into the app at
> build time with no independent version tag, a `normalize_arabic` rule change
> invalidates search caches only through the uthmani source-digest path and a
> redeploy. If independent search-cache invalidation is ever needed, reintroduce
> a pinned constant then.

**(B) `ByteStore` shape — document the sha256-tag design as accepted.**
The plan's §4.4 specified flattening storage to single-key `get(key)` /
`put(key, bytes)`. The implementation instead kept the two-axis
`get(tag, key)` shape, repurposing the axis to carry the per-file `sha256`.
**Decision: accept.** The sha256 tag is integrity, not versioning — it aligns
with the "keep per-file sha256" principle and never changes for a given
immutable DB, so it does not reintroduce the version-bump anti-pattern. (It also
avoids tripping the build-time guard at `web/scripts/assert-quran-data-boundary.ts:39`.)

**(C) Legacy migration — skipped; no installed base.**
`pruneOldVersions()` was deleted during the merge and no replacement
`migrateLegacyVersionedStorage()` was written, so a client that stored under an
old key would re-download once and strand the old entry. **Decision: skip.** The
application is unpublished, so the only client that ever held a version-keyed
build is the owner's own; a one-time re-download (clearable via site data) is
negligible, and there is no production installed base to migrate.

---

## 5. Integrity after removal (what keeps us safe)

Per-file sha256 — already implemented, retained unchanged, intact end-to-end:

- **Client download:** `download.ts:30` rejects on sha256 mismatch before the
  bytes are ever used.
- **Client cache hit:** `opfs-cache.ts:42` re-verifies on every OPFS read;
  `cached.ts:24-25` on every IDB read.
- **SSG build:** `quran-sqlite.ts:52-53` computes sha256 → `resolveSourceProfile`
  throws on mismatch (`source-profiles.ts:123-127`).
- **Rust boot:** `loader.rs:75-87` asserts the computed corpus digests against
  the pinned `GOLDEN_*` literals.
- **Served wire:** per-file `Artifact.sha256` on `/scripts`; corpus
  `source_digests` on `/health/ready`.

R2 object keys are already version-free (`source-profiles.ts:78,94`).

---

## 6. Decision log (preserved from the original plan)

- **(a) Delete, not freeze.** A frozen constant still costs the storage axis, the
  round-trip, the ETag plumbing, and the mental overhead. If the value never
  changes, the concept should not exist.
- **(b) Keep per-file sha256 as integrity.** Removing versioning must not weaken
  correctness — honored (§5).
- **(c) Migrate, don't drop, the boot assertion.** Honored: pinned per-file sha256,
  asserted at boot.
- **(d) `searchVersion` was to be kept.** *Overridden in implementation* — see §4 (A).
- **(e) Translations treated identically.** Honored — the versioning pattern was
  never multiplied into the translation pack design (§2.4).

---

## 7. Audit verification (2026-08-04)

`git grep 'content[_-]?[Vv]ersion'` across tracked source returns nothing in
`rust/backend/api/src/**` or `web/src/**`. Remaining doc mentions are either
negative assertions recording the removal (`quran-api.md:1019`,
`quran-ssg-optimization-plan.md:352`) or this document. The
`web/.svelte-kit/output/.../worker-client.js` chunk still references
`decodeVersionPayload`, but that is gitignored build output (not source);
`git check-ignore` confirms it, and it regenerates clean on the next build.
