# Hard rules:
- Quran databases are all immutable. No modifications and never versioning.
- Always load and follow the `caveman` skill at the beginning of every session. Keep Caveman full mode active for every response.

# Dynamic context loading
Load relevant context ONLY when working on that area:

- **Rust backend** (`rust/backend/api/`): load `rust/backend/api/AGENTS.md` + `rust/backend/api/README.md` if exists.
- **Web** (`web/`): load `web/README.md` + Svelte skills (`svelte-core-bestpractices`, `svelte-code-writer`).
- **DB/migrations**: load `rust/backend/api/migration/README.md`.
- **Deploy**: load `deploy/README.md`.

Do NOT preload all context every session. Load on-demand based on files touched.


