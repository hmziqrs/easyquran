# EasyQuran — root task runner for the web (SvelteKit) and api (Rust/Axum).
# Run `just` to list recipes. Requires `just`, `pnpm`, and `cargo` on PATH.

# Default: print the available recipes.
default:
    @just --list

# ── Setup ───────────────────────────────────────────────────────────────────

# Ensure rust/.env exists; created from .env.example with a generated COOKIE_KEY
# if missing. (Private; depended on by the api/migrate recipes.)
_env:
    @if [ ! -f rust/.env ]; then \
        cp rust/.env.example rust/.env; \
        KEY=$(openssl rand -hex 32); \
        perl -pi -e "s/^COOKIE_KEY=.*/COOKIE_KEY=$KEY/" rust/.env; \
        echo "Created rust/.env (COOKIE_KEY generated). Edit S3_/SMTP_* before running the api."; \
    fi

# One-time local setup: rust/.env + web deps. Safe to re-run.
setup: _env
    cd web && pnpm install

# ── Web (SvelteKit + Vite, in ./web) ─────────────────────────────────────────

web-install:
    cd web && pnpm install

# Vite dev server.
web-dev:
    cd web && pnpm dev

web-build:
    cd web && pnpm build

web-preview:
    cd web && pnpm preview

# svelte-check type-check.
web-check:
    cd web && pnpm check

# ── API (Rust/Axum on SQLite, in ./rust) ─────────────────────────────────────
# Recipes `cd rust/` so the binary's dotenvy picks up `rust/.env`
# (DATABASE_URL, COOKIE_KEY, S3_*, SMTP_*). `api-dev` also runs migrations on boot.

# Run the api (boots, runs pending migrations, serves on HOST:PORT from rust/.env).
api-dev: _env
    cd rust && cargo run -p ruxlog

# Debug build of the api binary.
api-build:
    cd rust && cargo build -p ruxlog

# Release build.
api-release:
    cd rust && cargo build -p ruxlog --release

# Type-check the whole Rust workspace (fast, no codegen).
api-check:
    cd rust && cargo check --workspace

# Run the Rust test suite.
api-test:
    cd rust && cargo test --workspace

# Apply pending SQLite migrations without starting the server.
api-migrate: _env
    cd rust && cargo run -p migration --bin migrate -- up

# Wipe and re-apply migrations against the dev DB. DANGER: deletes the file.
api-migrate-fresh db="data/easyquran.db": _env
    rm -f rust/{{db}}*
    cd rust && cargo run -p migration --bin migrate -- fresh

# Show migration status.
api-migrate-status: _env
    cd rust && cargo run -p migration --bin migrate -- status

# Lint (clippy) + format check.
api-lint:
    cd rust && cargo clippy --workspace --all-targets -- -D warnings
    cd rust && cargo fmt --all -- --check

api-fmt:
    cd rust && cargo fmt --all
