# EasyQuran — backend API

Axum 0.8 + sea-orm. SQLite only (no Postgres, no Redis). sea-orm migrations. In-memory TTL stores. Started as verbatim ruxlog clone -> EasyQuran API layered on top (binary still named `ruxlog`; rename deferred).

## Run

    just dev        # dev server
    just dev-w      # watch + reload
    just prod       # release build
    just migrate    # run migrations

SQLite file auto-created on open (`?mode=rwc`). Point `DATABASE_URL` at writable path.

## Env (`.env`)

    DATABASE_URL=sqlite:./data/easyquran.db?mode=rwc
    COOKIE_KEY=...
    SMTP_HOST=... SMTP_USERNAME=... SMTP_PASSWORD=...   # mail; SMTP works, SES pending

## Workspace

    rust/backend/api/
    ├─ src/
    │  ├─ main.rs            # boot, route nest, rate-limit layers
    │  ├─ router.rs state.rs constants.rs docs.rs
    │  ├─ modules/           # feature routes: auth, quran_v1, device, ...
    │  ├─ services/          # auth, mail, rate-limit, acl, image, ...
    │  └─ middlewares/       # csrf, permissions, status
    ├─ crates/
    │  ├─ rux-auth rux-fcm rux-provider-core rux-request-gate rux-webhook-crypto
    │  └─ ruxlog-types
    └─ migration/            # m000001_init (24 tables, SQLite DDL)

State = `AppState` (sea-orm pool + mailer), via `State` extractor. `auth_guard` uses `Extension<AppState>` (axum 0.8 `from_fn` can't take `State`). Sessions SQLite (`SqliteSessionStore`). No cache server.
