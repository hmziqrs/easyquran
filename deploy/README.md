# EasyQuran — deploy

Two images — **web** (SvelteKit SSG, served by Caddy) + **api** (Rust/Axum) — that
sit behind your **external Traefik** reverse proxy. No Traefik config lives here;
the containers carry the labels your proxy auto-discovers.

```
easyquran.fyi → [external Traefik] → web:8080   (Host)
                                  → api:8888    (Host + PathPrefix /api, stripped)
```

## Files

- `Dockerfile.web` / `Dockerfile.api` — the two images.
- `Caddyfile` — Caddy config for the web image (compression + cache headers +
  branded 404). Replaces the bare `caddy file-server` CMD.
- `../docker-compose.yml` (repo root) — web + api + Traefik labels, on the
  external proxy network. The canonical compose; used by both the Dokploy flow
  and the VPS cron flow below. It lives at the root because Dokploy writes/sources
  the `.env` relative to the compose file.
- `deploy.sh` — optional cron auto-deploy on a new `vX.Y.Z` tag.
- `../web/scripts/assert-headers.sh` — asserts the HTTP delivery contract
  against a running origin; run it before shipping (see checklist).
- `.env.example` — config (copy to `.env`).

## First deploy (on the VPS)

```bash
git clone <repo> /opt/easyquran && cd /opt/easyquran
cp deploy/.env.example .env
$EDITOR .env                 # DOMAIN, COOKIE_KEY (openssl rand -hex 32), PROXY_NETWORK…
# your external Traefik must already own the PROXY_NETWORK (e.g. `web`).
docker compose up -d --build
```

First build is slow (Rust compiles once; cached after). Verify:
`curl https://easyquran.fyi/api/healthz` → `{"status":"healthy"}`.

## Deploying with Dokploy

Dokploy runs Traefik internally on `dokploy-network` — so this stack drops in
with the defaults already in `.env.example` (`PROXY_NETWORK=dokploy-network`,
`HTTPS_ENTRYPOINT=websecure`, `ACME_RESOLVER=letsencrypt`; confirm the last two
against your Dokploy instance if you customized Traefik).

1. In Dokploy, create a **Docker Compose** application pointing at this repo.
2. Set the compose path to the **root `docker-compose.yml`** (not `deploy/…`).
   Dokploy writes the `.env` (from your Environment tab) and sources it relative
   to the compose file, so it must be at the repo root or the deploy fails with
   `…/code/deploy/.env: No such file or directory`.
3. **Don't** set a domain in the Dokploy UI — the `traefik.*` labels in
   `docker-compose.yml` already define the routing (Host + `/api` PathPrefix +
   stripPrefix). Dokploy runs `docker compose up`, attaches both services to
   `dokploy-network`, and its Traefik picks the labels up.
4. Prefer Dokploy's UI to manage routing instead? Delete the `traefik.*` labels
   and configure domains there: web → `easyquran.fyi`, api → `easyquran.fyi`
   with path `/api`.
5. In the Dokploy **Environment** tab, set `REVISION` to the short sha of the
   deployed commit (`git rev-parse --short=12 HEAD`). Dokploy runs
   `docker compose up` from its own checkout and does **not** export `REVISION`,
   so the compose `:?` guard intentionally fails the build until you set it —
   this guarantees every container image carries a unique, traceable
   `<tag>+<commit>` app version (see `web/vite.config.ts`). `VERSION` follows the
   `vX.Y.Z` tag automatically.

The common Dokploy gotcha (404s) is just containers not being on
`dokploy-network` — our compose attaches them, so you're covered.

The web image is served by Caddy from `deploy/Caddyfile` (compression +
`Cache-Control`/`X-Robots-Tag` headers + the branded 404 page), not the bare
`caddy file-server` CMD. That Caddyfile implements the static rows of the
[HTTP delivery contract](../docs/quran.md).

## Release

```bash
git tag v1.0.0 && git push origin v1.0.0
```

To auto-deploy, set up the cron (see `deploy.sh` header). It polls for a newer
`vX.Y.Z` tag, rebuilds web + api, and recreates only those two containers. A
failed build leaves the live site running. Without cron: `git pull && docker compose up -d --build`.

### Header checklist (before each release)

Run against the running web origin to confirm the delivery contract holds:

```bash
docker compose up -d --build web
web/scripts/assert-headers.sh http://localhost:8080
```

It verifies: `/_app/immutable/*` is `immutable`; `_app/version.json`, HTML
pages, and `__data.json` are `no-cache`; `*.md`/`*.txt` carry `X-Robots-Tag`;
brotli/gzip is offered; an unknown URL returns the branded 404; and (when Phase
3 lands) the offline pack + manifest are served correctly. Exits non-zero on any
hard failure.

## The dual API URL

| Surface | Var | Value |
|---|---|---|
| Browser | `PUBLIC_API_BASE_URL` | `https://easyquran.fyi/api` (baked into the bundle) |
| SSG build | `INTERNAL_API_BASE_URL` | `http://api:8888` (build-only, not shipped) |

The web build reads local SQLite only for SSG. Runtime artifact delivery always
uses R2 here — Docker images are production builds. The `/_quran` local artifact
source is a dev-server-only mode, selected with `PUBLIC_ENV=local`.

## Notes

- `/api` is stripped at the edge (`easyquran.fyi/api/healthz` → `/healthz`).
- The api binary is still named `ruxlog` (a ported backend) — cosmetic.
- VPS needs ≥2 GB RAM for the Rust build (add swap on smaller boxes).
