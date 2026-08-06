# EasyQuran — deploy

Two images — **web** (SvelteKit adapter-node: Arabic SSG + translation SSR) + **api** (Rust/Axum) — that
sit behind your **external Traefik** reverse proxy. No Traefik config lives here;
the containers carry the labels your proxy auto-discovers.

Both containers have health checks. Web `/health/quran` reports translated-page
disk-cache entries/bytes and hit/miss/write/eviction/error counters; API
`/quran/health/ready` reports corpus loading, resident database bytes, and the
bounded translation-pool metrics.

```
easyquran.fyi → [external Traefik] → web:8080   (Host)
                                  → api:8888    (Host + PathPrefix /api, stripped)
```

## Files

- `Dockerfile.web` / `Dockerfile.api` — the two images.
- `Dockerfile.web` runs the standalone adapter-node server **on the bun runtime**
  (`oven/bun:1.3.14-slim`), built on Node. The split is deliberate: prerendering reads the
  sqlite corpus via `node:sqlite`, which bun does not implement, and every sqlite route is
  `prerender = true` — so it is a build-time dependency the runtime never loads. bun holds
  roughly half Node's resident memory under translation SSR (see `bench/`). Its persistent
  `web_quran_cache` volume stores only translated-page HTML (7-day TTL, 256 MiB LRU budget).
- `../docker-compose.yml` (repo root) — web + api + Traefik labels, on the
  external proxy network. The canonical compose; used by the Dokploy flow. It
  lives at the root because Dokploy writes/sources the `.env` relative to the
  compose file.
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

The common Dokploy gotcha (404s) is just containers not being on
`dokploy-network` — our compose attaches them, so you're covered.

The web image runs SvelteKit's standalone adapter-node server on bun. `hooks.server.ts` owns
translated-page disk caching and dynamic response headers; `server.ts` applies
the same `Cache-Control`/`X-Robots-Tag` contract to adapter-node's static bypass. Traefik's
compression middleware handles dynamic SSR while adapter-node serves precompressed
Arabic output and immutable assets.

## Release

Dokploy rebuilds automatically on every push to the tracked branch — just push.
Tag releases for your own record:

```bash
git tag v1.0.0 && git push origin v1.0.0
```

### Header checklist (before each release)

Run against the running web origin to confirm the delivery contract holds:

```bash
docker compose up -d --build
web/scripts/assert-headers.sh http://localhost:8080 http://localhost:8888
```

It verifies: `/_app/immutable/*` and allowlisted `/_quran/*` artifacts are `immutable`; `_app/version.json`, HTML
pages, and `__data.json` are `no-cache`; `*.md`/`*.txt` carry `X-Robots-Tag`;
brotli/gzip is offered; translated SSR goes cold → warm through the disk cache;
the API exposes Quran pool metrics; an unknown URL returns the branded 404; and
the offline pack + manifest are served correctly. Exits non-zero on any hard failure.

## The dual API URL

| Surface | Var | Value |
|---|---|---|
| Browser | `PUBLIC_API_BASE_URL` | `https://easyquran.fyi/api` (baked into the bundle) |
| Browser Quran | `PUBLIC_QURAN_API_BASE` | `https://easyquran.fyi/api/quran` (baked into the bundle) |
| Node Quran SSR | `INTERNAL_QURAN_API_BASE` | `http://api:8888/quran` (runtime-private) |
| Other server work | `INTERNAL_API_BASE_URL` | `http://api:8888` (runtime-private) |

The web build reads local Uthmani SQLite only for Arabic SSG. Translation SSR
reads the internal Axum API. Browser OPFS downloads use the web origin's strict
`/_quran/tanzil/*` gateway, which streams only baked artifact keys from R2 and
forwards range requests. Docker images are production builds. In local mode,
Vite serves those same URLs directly from tracked immutable artifacts.

## Notes

- `/api` is stripped at the edge (`easyquran.fyi/api/healthz` → `/healthz`).
- API image contains root-owned, filesystem-read-only Quran source files; databases are never written.
- `web_quran_cache` is disposable derived HTML; removing it causes cold SSR only.
- The api binary is still named `ruxlog` (a ported backend) — cosmetic.
- VPS needs ≥2 GB RAM for the Rust build (add swap on smaller boxes).
