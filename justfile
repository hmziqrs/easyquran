set dotenv-load := true

# Image coordinates. Override per-invocation (`REGISTRY=... just images v1.2.0`) or in .env.
REGISTRY := env_var_or_default("REGISTRY", "ghcr.io/hmziqrs")
# Deploy target is an arm64 Ubuntu server; the build host is Apple Silicon. Same arch, so
# every image below builds natively — no qemu, no emulation.
PLATFORM := "linux/arm64"
# Cross-compile triple for the Rust binary. `.2.36` pins the glibc floor (Debian 12 /
# Ubuntu 22.04 and newer), independent of whatever glibc the runtime base image ships.
RUST_TARGET := "aarch64-unknown-linux-gnu"

default:
    @just --list

_env:
    @if [ ! -f .env ]; then \
        cp .env.example .env; \
        KEY=$(openssl rand -hex 32); \
        perl -pi -e "s/^COOKIE_KEY=.*/COOKIE_KEY=$KEY/" .env; \
        echo "Created .env (COOKIE_KEY generated). Edit S3_/SMTP_* before running."; \
    fi
    @ln -sf ../.env web/.env
    @ln -sf ../.env rust/.env

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

# --- Images -------------------------------------------------------------------------------
# Both images are packaging-only: everything is compiled HERE, on the host, and the
# Dockerfiles just COPY the artifacts in. Nothing installs a toolchain or hits the network
# inside Docker anymore, which is what kept breaking. Dokploy no longer builds — it pulls the
# tags pushed by `just push`.

# SvelteKit build (host) -> web image. ~10s of Docker on top of the normal pnpm build.
image-web version="dev":
    just web-build prod
    docker build --platform {{PLATFORM}} -f deploy/Dockerfile.web \
        --build-arg VERSION={{version}} \
        -t {{REGISTRY}}/easyquran-web:{{version}} \
        -t {{REGISTRY}}/easyquran-web:latest .

# Cross-compiled ruxlog (host, cargo-zigbuild) -> api image. Needs `zig` + `cargo-zigbuild`
# (brew install zig; cargo install cargo-zigbuild) and `rustup target add {{RUST_TARGET}}`.
# `vendored-openssl` builds OpenSSL from source: webauthn-rs needs it and pkg-config cannot
# find a target libssl when cross-compiling. First build is slow, then it caches.
image-api version="dev":
    cd rust && cargo zigbuild --release --locked -p ruxlog \
        --features vendored-openssl --target {{RUST_TARGET}}.2.36
    docker build --platform {{PLATFORM}} -f deploy/Dockerfile.api \
        --build-arg VERSION={{version}} \
        -t {{REGISTRY}}/easyquran-api:{{version}} \
        -t {{REGISTRY}}/easyquran-api:latest .

images version="dev": (image-web version) (image-api version)

# Push both tags. Requires `docker login ghcr.io` once (PAT with write:packages).
push version="dev": (images version)
    docker push {{REGISTRY}}/easyquran-web:{{version}}
    docker push {{REGISTRY}}/easyquran-web:latest
    docker push {{REGISTRY}}/easyquran-api:{{version}}
    docker push {{REGISTRY}}/easyquran-api:latest

docker-up environment="local":
    @case "{{environment}}" in local|prod) ;; *) echo "environment must be local or prod (received: {{environment}})" >&2; exit 2;; esac
    just images latest
    @if [ "{{environment}}" = "local" ]; then \
        docker network inspect easyquran-local >/dev/null 2>&1 || docker network create easyquran-local; \
        PROJECT=easyquran PROXY_NETWORK=easyquran-local DOMAIN=localhost \
        docker compose -f docker-compose.yml -f docker-compose.local.yml up; \
    else \
        docker compose -f docker-compose.yml up; \
    fi
