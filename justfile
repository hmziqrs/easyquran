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

# The Quran data environment is public and controls where the browser gets the
# verified SQLite artifacts: local = repository files, prod = Cloudflare R2.
# `web-preview` rebuilds first so its source matches the requested environment.
# Examples: `just web-dev local`, `just web-build prod`, `just web-preview local`.
web-dev environment="local":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    cd web && PUBLIC_ENV="{{environment}}" pnpm dev

web-build environment="prod":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    cd web && PUBLIC_ENV="{{environment}}" pnpm build

web-preview environment="prod":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    cd web && PUBLIC_ENV="{{environment}}" pnpm build
    cd web && pnpm preview

# svelte-check type-check.
web-check:
    cd web && pnpm check

# Measure SSG output sizes (raw/gzip/brotli HTML, page-data, critical-JS modulepreload graph).
# Run after `web-build`; pass a stable phase label for the checked-in comparison.
web-baseline label="phase0":
    cd web && node scripts/measure-baselines.ts "{{label}}"

# Prove compact Quran metadata is neither embedded in generated output nor SW-precached.
web-assert-metadata:
    cd web && node scripts/assert-quran-metadata-boundary.ts

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

# ── Quran data pipeline (TS in ./db/quran/tanzil/translations) ───────────────
# Loads R2_* creds from the root .env, then runs the SQLite upload to R2.
#   just upload-sqlite --dry-run   # preview, no creds needed
#   just upload-sqlite             # upload, skip already-present
#   just upload-sqlite --force     # re-upload everything
upload-sqlite *args='':
    @if [ -f ./.env ]; then set -a && . ./.env && set +a; fi; \
    cd db/quran/tanzil/translations && npm run upload:sqlite -- {{args}}

# Docker always builds production: Quran artifacts come from R2. `local` only
# changes exposure (localhost port override + a local proxy network).
docker-up environment="local":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    @if [ "{{environment}}" = "local" ]; then \
        docker network inspect easyquran-local >/dev/null 2>&1 || docker network create easyquran-local; \
        PROJECT=easyquran PROXY_NETWORK=easyquran-local DOMAIN=localhost \
        docker compose -f docker-compose.yml -f docker-compose.local.yml up --build; \
    else \
        docker compose -f docker-compose.yml up --build; \
    fi
