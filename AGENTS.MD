# Hard rules:
- Quran databases are all immutable. No modifications and never versioning.
- SHA-256 is a **manual human-audit tool only** — never in build, boot, runtime, ETag, source-profile, or catalogue. A Quran database's identity is its **id**. The only places SHA-256 over Quran data is computed or compared are three commands a human runs intentionally: `just quran-audit` (Rust golden corpus + file digests), `pnpm audit:arabic` (web Arabic sqlite digests), and `pnpm verify` in `db/quran/tanzil/translations` (translation build digests). Re-wiring sha into an automated path or a cache/ETag/catalogue key breaks `rust tests/quran_v1.rs` (`sources_rows_never_carry_sha256`, `scripts_endpoint_carries_no_sha256`) and `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts` — do not silence them; fix the regression (docs/quran.md §2). Generic crypto primitives (CSRF `hkdf_sha256`, webhook HMAC, Argon2id, PKCE) are unrelated to Quran data and stay.
- Always load and follow the `caveman` skill at the beginning of every session. Keep Caveman full mode active for every response. This rule is also applied to subagents.

# Dynamic context loading
Load relevant context ONLY when working on that area:

- **Rust backend** (`rust/backend/api/`): load `rust/backend/api/AGENTS.md` + `rust/backend/api/README.md` if exists.
- **Web** (`web/`): load `web/README.md` + Svelte skills (`svelte-core-bestpractices`, `svelte-code-writer`).
- **DB/migrations**: load `rust/backend/api/migration/README.md`.
- **Deploy**: load `deploy/README.md`.

Do NOT preload all context every session. Load on-demand based on files touched.


