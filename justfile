default:
    @just --list

_env:
    @if [ ! -f rust/.env ]; then \
        cp rust/.env.example rust/.env; \
        KEY=$(openssl rand -hex 32); \
        perl -pi -e "s/^COOKIE_KEY=.*/COOKIE_KEY=$KEY/" rust/.env; \
        echo "Created rust/.env (COOKIE_KEY generated). Edit S3_/SMTP_* before running the api."; \
    fi

setup: _env
    cd web && pnpm install

web-install:
    cd web && pnpm install

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

web-check:
    cd web && pnpm check

web-baseline label="phase0":
    cd web && node scripts/measure-baselines.ts "{{label}}"

web-assert-quran-data:
    cd web && node scripts/assert-quran-data-boundary.ts

api-dev: _env
    cd rust && cargo run -p ruxlog

api-build:
    cd rust && cargo build -p ruxlog

api-release:
    cd rust && cargo build -p ruxlog --release

api-check:
    cd rust && cargo check --workspace

api-test:
    cd rust && cargo test --workspace

api-migrate: _env
    cd rust && cargo run -p migration --bin migrate -- up

api-migrate-fresh db="data/easyquran.db": _env
    rm -f rust/{{db}}*
    cd rust && cargo run -p migration --bin migrate -- fresh

api-migrate-status: _env
    cd rust && cargo run -p migration --bin migrate -- status

api-lint:
    cd rust && cargo clippy --workspace --all-targets -- -D warnings
    cd rust && cargo fmt --all -- --check

api-fmt:
    cd rust && cargo fmt --all

upload-sqlite *args='':
    @if [ -f ./.env ]; then set -a && . ./.env && set +a; fi; \
    cd db/quran/tanzil/translations && npm run upload:sqlite -- {{args}}

docker-up environment="local":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    @if [ "{{environment}}" = "local" ]; then \
        docker network inspect easyquran-local >/dev/null 2>&1 || docker network create easyquran-local; \
        PROJECT=easyquran PROXY_NETWORK=easyquran-local DOMAIN=localhost \
        docker compose -f docker-compose.yml -f docker-compose.local.yml up --build; \
    else \
        docker compose -f docker-compose.yml up --build; \
    fi
