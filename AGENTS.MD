# Hard rules:
- Quran databases are all immutable. No modifications and never versioning.
- No SHA-256 over Quran data in any automated path (build, boot, runtime, ETag, cache, catalogue). A DB's identity is its **id**, never a hash. SHA-256 is manual audit only — three commands a human runs: `just quran-audit`, `pnpm audit:arabic`, `pnpm verify` (in `db/quran/tanzil/translations`). Crypto sha (CSRF, HMAC, Argon2, PKCE) is unrelated and stays. Don't re-add Quran sha — guards in `tests/quran_v1.rs` + `web catalogue-sha-guard.test.ts` fail the build (see docs/quran.md §2).
- Always load and follow the `caveman` skill at the beginning of every session. Keep Caveman full mode active for every response. This rule is also applied to subagents.

# Dynamic context loading
Load relevant context ONLY when working on that area:

- **Rust backend** (`rust/backend/api/`): load `rust/backend/api/AGENTS.md` + `rust/backend/api/README.md` if exists.
- **Web** (`web/`): load `web/README.md` + Svelte skills (`svelte-core-bestpractices`, `svelte-code-writer`).
- **DB/migrations**: load `rust/backend/api/migration/README.md`.
- **Deploy**: load `deploy/README.md`.

Do NOT preload all context every session. Load on-demand based on files touched.


