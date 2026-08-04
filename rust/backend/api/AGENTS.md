# Backend API — agent rules

EasyQuran Axum API (ruxlog clone). SQLite only — no Postgres, no Redis. sea-orm + in-memory TTL stores.

## Hard rule
Quran databases immutable. No modifications, no versioning. (See repo-root `AGENTS.MD`.)

## Commands
    just dev          # run  (from repo root: just api-dev env=dev)
    just dev-w        # watch + reload
    just prod         # release
    just migrate      # sea-orm migrations (m000001_init)
    cargo test
    cargo fmt
    cargo clippy --all-targets -D warnings   # before commit

## Structure
    src/main.rs         # boot, route nest, rate-limit layers
    src/router.rs state.rs constants.rs docs.rs
    src/modules/*_v1/   # feature routes: auth, quran_v1, device, ...
    src/services/       # auth, mail, rate-limit, acl, image, ...
    src/middlewares/    # csrf, permissions, status
    crates/             # rux-auth, rux-fcm, rux-provider-core, rux-request-gate, rux-webhook-crypto, ruxlog-types
    migration/          # sea-orm, SQLite DDL

## Gotchas
- `auth_guard` uses `Extension<AppState>`, not `State` — axum 0.8 `from_fn` can't take `State`.
- Sessions SQLite (`SqliteSessionStore`). No cache server.
- DB schema change -> add a migration in `migration/`, call out in PR.
