# Hard rules:
- Quran databases are all immutable. No modifications and never versioning.
- Never use SHA-256 as a Quran catalogue, cache, or identity key. A database's identity is its **id**. The only SHA-256 that stays: (a) Arabic integrity (`source-profiles` golden digests + `/scripts`), (b) build-time `db/.../verify.ts`, (c) crypto primitives (CSRF/HMAC/Argon/PKCE). Re-wiring sha into a catalogue field or cache dir breaks `rust tests/quran_v1.rs::sources_rows_never_carry_sha256` and `web catalogue-sha-guard.test.ts` — these enforce docs/quran.md §2; do not silence them.
- Always load and follow the `caveman` skill at the beginning of every session. Keep Caveman full mode active for every response. This rule is also applied to subagents.

# Dynamic context loading
Load relevant context ONLY when working on that area:

- **Rust backend** (`rust/backend/api/`): load `rust/backend/api/AGENTS.md` + `rust/backend/api/README.md` if exists.
- **Web** (`web/`): load `web/README.md` + Svelte skills (`svelte-core-bestpractices`, `svelte-code-writer`).
- **DB/migrations**: load `rust/backend/api/migration/README.md`.
- **Deploy**: load `deploy/README.md`.

Do NOT preload all context every session. Load on-demand based on files touched.


