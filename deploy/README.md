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
- `docker-compose.yml` — web + api + Traefik labels, on the external proxy network.
- `deploy.sh` — optional cron auto-deploy on a new `vX.Y.Z` tag.
- `.env.example` — config (copy to `.env`).

## First deploy (on the VPS)

```bash
git clone <repo> /opt/easyquran && cd /opt/easyquran
cp deploy/.env.example deploy/.env
$EDITOR deploy/.env          # DOMAIN, COOKIE_KEY (openssl rand -hex 32), PROXY_NETWORK…
# your external Traefik must already own the PROXY_NETWORK (e.g. `web`).
docker compose -f deploy/docker-compose.yml up -d --build
```

First build is slow (Rust compiles once; cached after). Verify:
`curl https://easyquran.fyi/api/healthz` → `{"status":"healthy"}`.

## Deploying with Dokploy

Dokploy runs Traefik internally on `dokploy-network` — so this stack drops in
with the defaults already in `.env.example` (`PROXY_NETWORK=dokploy-network`,
`HTTPS_ENTRYPOINT=websecure`, `ACME_RESOLVER=letsencrypt`; confirm the last two
against your Dokploy instance if you customized Traefik).

1. In Dokploy, create a **Docker Compose** application pointing at this repo.
2. **Don't** set a domain in the Dokploy UI — the `traefik.*` labels in
   `docker-compose.yml` already define the routing (Host + `/api` PathPrefix +
   stripPrefix). Dokploy runs `docker compose up`, attaches both services to
   `dokploy-network`, and its Traefik picks the labels up.
3. Prefer Dokploy's UI to manage routing instead? Delete the `traefik.*` labels
   and configure domains there: web → `easyquran.fyi`, api → `easyquran.fyi`
   with path `/api`.

The common Dokploy gotcha (404s) is just containers not being on
`dokploy-network` — our compose attaches them, so you're covered.

## Release

```bash
git tag v1.0.0 && git push origin v1.0.0
```

To auto-deploy, set up the cron (see `deploy.sh` header). It polls for a newer
`vX.Y.Z` tag, rebuilds web + api, and recreates only those two containers. A
failed build leaves the live site running. Without cron: `git pull && docker
compose -f deploy/docker-compose.yml up -d --build`.

## The dual API URL

| Surface | Var | Value |
|---|---|---|
| Browser | `PUBLIC_API_BASE_URL` | `https://easyquran.fyi/api` (baked into the bundle) |
| SSG build | `INTERNAL_API_BASE_URL` | `http://api:8888` (build-only, not shipped) |

The web build reads local `quran-uthmani.sqlite` for SSG today; the internal URL
is forward-ready for the live `/quran/v1` API.

## Notes

- `/api` is stripped at the edge (`easyquran.fyi/api/healthz` → `/healthz`).
- The api binary is still named `ruxlog` (a ported backend) — cosmetic.
- VPS needs ≥2 GB RAM for the Rust build (add swap on smaller boxes).
