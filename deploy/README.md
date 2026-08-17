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

## Images are built on your machine, never in the deploy

Both Dockerfiles are **packaging-only**: they compile nothing, install no toolchain, and make
no network calls. Everything is built on a dev machine and COPYed in.

```bash
just images v1.2.0     # host build + both images (tags: v1.2.0 and latest)
just push   v1.2.0     # push to $REGISTRY (default ghcr.io/hmziqrs)
```

Why: the old `Dockerfile.web` re-installed pnpm + the whole toolchain, needed CA certs added
to `node:24-slim`, and re-ran the sqlite prerender inside the image on every build. That step
failed repeatedly and opaquely (a bare `Exit status 1` with no error text). It is gone.

Requirements on the build host (Apple Silicon → arm64 Ubuntu server, so same arch, no qemu):

- `zig` + `cargo-zigbuild` (`brew install zig`, `cargo install cargo-zigbuild`)
- `rustup target add aarch64-unknown-linux-gnu`
- `docker login ghcr.io` once, with a PAT carrying `write:packages`

What each recipe does:

| recipe | host step | image step |
| --- | --- | --- |
| `just image-web` | `pnpm --filter web build` (PUBLIC_ENV=prod) | COPY `web/build` + `web/server.ts` (~10s) |
| `just image-api` | `cargo zigbuild --target aarch64-unknown-linux-gnu.2.36 --features vendored-openssl` | COPY the one binary |

`vendored-openssl` exists only for the cross-build: `webauthn-rs` pulls `openssl-sys`
unconditionally and pkg-config cannot resolve a target libssl when cross-compiling, so
OpenSSL is compiled from vendored source instead. Native `cargo build` is unaffected.
`.2.36` pins the glibc floor (Debian 12 / Ubuntu 22.04+).

Registry coordinates are `${REGISTRY:-ghcr.io/hmziqrs}/easyquran-{web,api}:${VERSION:-latest}`,
resolved identically by the justfile and `docker-compose.yml`. If the packages are private,
the server needs `docker login ghcr.io` too (or a Dokploy registry entry) — otherwise `up`
fails on pull.

## Files

- `Dockerfile.web` / `Dockerfile.api` — the two images, plus a
  `Dockerfile.<name>.dockerignore` each. BuildKit prefers the per-Dockerfile ignore over the
  root `.dockerignore`, which is what lets these builds see `web/build` and `rust/target/…`
  (both blanket-excluded at the root) while shipping nothing else.
- `Dockerfile.web` runs the standalone adapter-node server **on the bun runtime**
  (`oven/bun:1.3.14-slim`). The SvelteKit build still runs on Node, just on the host now:
  prerendering reads the sqlite corpus via `node:sqlite`, which bun does not implement, and
  every sqlite route is `prerender = true` — a build-time dependency the runtime never loads.
  bun holds roughly half Node's resident memory under translation SSR (see `bench/`). Its
  persistent `web_quran_cache` volume stores only translated-page HTML (7-day TTL, 256 MiB
  LRU budget).
- `../docker-compose.yml` (repo root) — web + api + Traefik labels, on the
  external proxy network. The canonical compose; used by the Dokploy flow. It
  lives at the root because Dokploy writes/sources the `.env` relative to the
  compose file.
- `fetch-quran-db.sh` (`just quran-fetch [arabic|all]`) — provisions the immutable Quran
  databases into gitignored `db/` from the public R2 base: read-only HTTPS GETs, no
  credentials, keys taken from the baked maps. `arabic` is what the web build's prerender
  needs; `all` adds the translation sqlites the api serves off its bind mount. Publishing
  remains `just upload-sqlite`.
- `../web/scripts/assert-headers.sh` — asserts the HTTP delivery contract
  against a running origin; run it before shipping (see checklist).
- `.env.example` — config (copy to `.env`).

## First deploy (on the VPS)

```bash
git clone <repo> /opt/easyquran && cd /opt/easyquran
cp deploy/.env.example .env
$EDITOR .env                 # DOMAIN, COOKIE_KEY, FIELD_ENC_KEY, INTERNAL_QURAN_API_TOKEN, ALLOWED_ORIGINS
# Proxy topology defaults to stock Dokploy (PROXY_NETWORK/HTTPS_ENTRYPOINT/ACME_RESOLVER
# in docker-compose.yml) — override in .env only if your Traefik is non-stock.
docker login ghcr.io          # only if the packages are private
docker compose up -d
```

The server builds nothing — it pulls the tags you pushed with `just push`. Verify the public
Quran route: `curl https://easyquran.fyi/api/quran/health/ready` → `{"ready":true,…}`.
`/healthz` is deliberately **not** on the public router — reach it from the
Docker network (`curl http://api:8888/healthz`) or the in-container healthcheck.

## Deploying with Dokploy

Dokploy never builds: the Compose application only **pulls** the registry images
(`pull_policy: always`) that CI (`.github/workflows/images.yml`) pushes on every
master push/tag — `ghcr.io/hmziqrs/easyquran-{web,api}`. One-time setup:

### 0. Prerequisites

- A Dokploy instance with its stock Traefik — the compose defaults already match
  (`PROXY_NETWORK=dokploy-network`, `HTTPS_ENTRYPOINT=websecure`,
  `ACME_RESOLVER=letsencrypt`). Override in the env tab only if you customized
  Traefik. The common Dokploy 404 gotcha is containers not on `dokploy-network`;
  the compose attaches them, so you're covered.
- ghcr packages readable from the server: either make both packages **public**
  (GitHub → package → settings), or add a Dokploy registry entry (Settings →
  Registries, ghcr.io, a PAT with `read:packages`).

### 1. Provision the Quran DBs outside the checkout (once, on the VPS)

Dokploy owns the code directory and re-syncs it per deploy, so the ~130 MB of
gitignored databases cannot live inside it. Put them on a stable path and point
both the fetch and the mount at it:

```bash
ssh <vps>
export QURAN_DB_DIR=/opt/easyquran/quran        # stable; survives redeploys
git clone --depth 1 https://github.com/hmziqrs/easyquran /tmp/easyquran
cd /tmp/easyquran && QURAN_DB_DIR="$QURAN_DB_DIR" deploy/fetch-quran-db.sh all
rm -rf /tmp/easyquran
```

`all` = the Arabic corpus + every translation sqlite (the api serves
translations off this mount). Credential-free GETs from the public R2 base;
node or python3 on the host reads the catalogue. Set the **same** `QURAN_DB_DIR`
in the env tab (step 2) — compose bind-mounts it read-only into the api.

### 2. Create the Compose application

1. Dokploy → project → new **Docker Compose** app pointing at this repo,
   branch `master` (private repo: connect Dokploy's GitHub provider first).
2. Compose path: the **root `docker-compose.yml`** (not `deploy/…`). Dokploy
   writes the `.env` (from your Environment tab) and sources it relative to the
   compose file, so it must be at the repo root or the deploy fails with
   `…/code/deploy/.env: No such file or directory`.
3. Environment tab: the 5 required vars from `.env.example` (`DOMAIN`,
   `COOKIE_KEY`, `FIELD_ENC_KEY`, `INTERNAL_QURAN_API_TOKEN`,
   `ALLOWED_ORIGINS` — literal CSV, no `${DOMAIN}` expansion) plus
   `QURAN_DB_DIR` from step 1.
4. **Don't** set a domain in the Dokploy UI — the `traefik.*` labels already
   define the routing (Host + `/api` PathPrefix + stripPrefix). Prefer the UI
   anyway? Delete the labels and configure domains there: web → `easyquran.fyi`,
   api → `easyquran.fyi` with path `/api`.

### 3. Wire auto-redeploy (GitHub repo secrets)

Set exactly one of these in the GitHub repo's secrets — CI's deploy job pokes it
after **both** images are pushed:

- `DOKPLOY_DEPLOY_WEBHOOK` — the compose app's Webhook URL (Dokploy app →
  Webhooks tab). Simplest; recommended.
- or the API trio `DOKPLOY_URL` (your Dokploy base URL) + `DOKPLOY_API_KEY` +
  `DOKPLOY_COMPOSE_ID` (the UUID in the compose app's URL) → `POST /api/compose.deploy`.

Neither set → CI warns and skips the redeploy (images still pushed).

### 4. First deploy + verify

Deploy from the Dokploy UI, wait for both services to report healthy, then:

```bash
curl https://easyquran.fyi/api/quran/health/ready   # → {"ready":true,…}
```

Ongoing: every master push → CI builds both images (`:latest` + `:sha`), pushes,
and redeploys. Tag pushes ship `v*` tags — pin `VERSION=v1.0.0` in the env tab
to freeze on a release instead of tracking `latest`.

The web image runs SvelteKit's standalone adapter-node server on bun. `hooks.server.ts` owns
translated-page disk caching and dynamic response headers; `server.ts` applies
the same `Cache-Control`/`X-Robots-Tag` contract to adapter-node's static bypass. Traefik's
compression middleware handles dynamic SSR while adapter-node serves precompressed
Arabic output and immutable assets.

## Release

Dokploy no longer builds anything. Images ship either from CI
(`.github/workflows/images.yml`) or by hand from a dev machine:

```bash
just push v1.0.0                  # host build → both images → registry
git tag v1.0.0 && git push origin v1.0.0
```

then redeploy in Dokploy (or `docker compose pull && docker compose up -d` on the box). Set
`VERSION=v1.0.0` in the environment to pin a release instead of tracking `latest`.

### Header checklist (before each release)

Run against the running web origin to confirm the delivery contract holds:

```bash
just docker-up local          # builds both images on the host, then `compose up`
web/scripts/assert-headers.sh http://localhost:8080 http://localhost:8888
```

It verifies: `/_app/immutable/*` and allowlisted `/_quran/*` artifacts are `immutable`; `_app/version.json`, HTML
pages, and `__data.json` are `no-cache`; `*.md`/`*.txt` carry `X-Robots-Tag`;
brotli/gzip is offered; translated SSR goes cold → warm through the disk cache;
the API exposes Quran pool metrics; an unknown URL returns the branded 404; and
the offline pack + manifest are served correctly. Exits non-zero on any hard failure.

## API URL resolution

All four used to be env vars. With web+api co-located on one Docker network they
collapse: the internal base is a hardcoded invariant, the public bases derive
from `DOMAIN`, and the unused `INTERNAL_API_BASE_URL` is deleted.

| Surface | Where set | Value |
|---|---|---|
| Browser | `PUBLIC_API_BASE_URL` in compose | `https://${DOMAIN}/api` (runtime `$env/dynamic/public`, derived from `DOMAIN`) |
| Browser Quran | `PUBLIC_QURAN_API_BASE` in compose | `https://${DOMAIN}/api/quran` (runtime, derived) |
| SSR Quran | `INTERNAL_QURAN_API_BASE` in compose | `http://api:8888/quran` (co-located invariant, literal) |

`PUBLIC_*` are read at runtime via `$env/dynamic/public` (not build-baked), so
they live in the web container's `environment:`, not in build args.

## Ingress identity contract (W2)

External rate limiting keys on the **verified Cloudflare client identity**, not
on the proxy's TCP address. Three isolated, non-escalating identities:

| Identity | How resolved | Bucket | Limiter |
|---|---|---|---|
| External client | `CF-Connecting-IP` header (parsed `IpAddr`) | `ratelimit:{ip}:…` | content ceiling (600/min) — the only one that can enter W3a escalation |
| Trusted internal SSR (Bun) | server-only `X-EasyQuran-Internal-Token`, constant-time match | `ratelimit:internal-webssr:…` | `QURAN_INTERNAL_REQUESTS_PER_MINUTE` (default 600) |
| Public readiness | exempt from identity resolution | `ratelimit:unknown:quran-health` | `QURAN_HEALTH_REQUESTS_PER_MINUTE` (default 120) |
| Docker health (`/healthz`) | exempt from identity, route blocker, and all limiters; **not on the public host router** (Docker-network / in-container only) | — | — |

`.env.example` ships `IP_SOURCE=CfConnectingIp` (exact PascalCase; `connect-info`
and `cf-connecting-ip` are invalid). The api refuses to boot in production with
`IP_SOURCE=ConnectInfo` — behind Cloudflare→Traefik that would collapse every
caller into one proxy-IP bucket. Production boot also fails closed on a missing
`INTERNAL_QURAN_API_TOKEN` or a non-positive internal/health limit.

### Generate the internal token

```bash
# Per deployment. Server-only — NEVER under a PUBLIC_ var, never in a response or log.
openssl rand -hex 32   # → INTERNAL_QURAN_API_TOKEN in .env
```

Compose interpolates it into both containers: the api compares it
constant-time in its identity gate; the web (Bun) container reads it from private
runtime env and sends it only to `INTERNAL_QURAN_API_BASE` on
`X-EasyQuran-Internal-Token` (`web/src/lib/server/quran-translation-page.ts`).

### Restrict origin ingress to Cloudflare ranges (required before production)

`CfConnectingIp` **trusts** the header. The api cannot tell a forged
`CF-Connecting-IP` from Cloudflare's own — only the network path can. Before
this configuration reaches production, Traefik (or the host firewall) MUST deny
public origin ingress from anywhere except current Cloudflare ranges, so a
public caller can never choose the header value.

Range-update procedure (owner-run; the ranges are not part of this repo):

1. Fetch the current Cloudflare IPv4/IPv6 lists:
   `https://www.cloudflare.com/ips-v4` and `https://www.cloudflare.com/ips-v6`.
2. Configure your external Traefik (the one on `PROXY_NETWORK`) to permit origin
   traffic (the `api` and `web` upstreams) only from those CIDRs; deny direct
   origin access from any other source.
3. Re-run on a schedule — Cloudflare publishes range changes a few times a year.

### Direct-origin negative test (owner-run after restricting ranges)

From a host that is **not** behind Cloudflare (e.g. a direct curl from your laptop
to the origin IP, bypassing Traefik), confirm the contract holds:

```bash
export INTERNAL_QURAN_API_TOKEN="$(grep ^INTERNAL_QURAN_API_TOKEN= .env | cut -d= -f2-)"
web/scripts/assert-headers.sh https://easyquran.fyi https://easyquran.fyi/api
```

It proves: an external request without `CF-Connecting-IP` is rejected at origin
(400), a forged internal token gets no privilege, the Docker health route and
public readiness stay exempt from the identity gate, and a valid internal token
enters the service bucket. The live CF-range negative test (a non-CF origin
cannot set `CF-Connecting-IP`) is the owner's sign-off; this script is the
scaffolding.

## Ban inspection & export (`/admin/bans`)

The API owns IP-ban enforcement in-process (L1) with SQLite durability (L2).
Three operator routes, all behind the edge `/api` strip and the session/CSRF/origin
stack of the private router:

- `GET /api/admin/bans` — list active bans (admin session ACL; `Cache-Control: no-store`; paginated `?page=&perPage=`).
- `DELETE /api/admin/bans` — lift a ban (admin session ACL; JSON body `{"banUnit":"203.0.113.5/32"}`). Deletes the exact L2 `quran-ban:{unit}` + `ratelimit:{unit}:quran-v1` rows in one transaction, then clears the matching L1 buckets and suspicious history. A DB failure returns failure **without** clearing L1, so the ban stays enforced and the delete is safe to retry.
- `GET /api/admin/bans/export` — neutral JSON feed for a deployment-owned proxy adapter. Human access uses the admin session ACL; machine access uses `Authorization: Bearer $BAN_EXPORT_TOKEN` (compared constant-time; set via `BAN_EXPORT_TOKEN`, generate with `openssl rand -hex 32`). The token is **never** accepted by the list/delete routes.

A ban unit is one canonical value: an IPv4 address → `a.b.c.d/32`, an IPv6 address → its `/64` network (host bits truncated). Export emits **only** active `quran-ban:` rows whose suffix parses as a valid `/32` or `/64` unit — email, user-id, totp, and fixed-rate keys are never exported. Export shape:

```json
{ "bans": [ { "banUnit": "203.0.113.5/32", "scope": "Long", "expiresAt": 1735689600 } ] }
```

`expiresAt` is a UNIX timestamp (seconds); `scope` is `"Temp"` or `"Long"`.

**Proxy adapter contract (outside this repo).** A Traefik/Cloudflare middleware
that consumes this export is a deployment-owned component — none lives in this
repository, and an existing export row is **not** proof that an edge block is in
place. Any adapter MUST preserve both the expiry timestamp and the un-ban
semantics: lift the edge block no later than `expiresAt`, and re-poll export
after an operator `DELETE /admin/bans` so a lifted ban is not kept at the edge.
The export never contains PII (no email, user-id, or raw path) — only canonical
CIDR units.

## Mail delivery smoke test (before enabling WEB_AUTH_ENABLED)

Production boot fails closed when `WEB_AUTH_ENABLED=true` unless `MAIL_PROVIDER`
is a real transport with credentials and a non-empty `MAIL_FROM_ADDRESS` /
`MAIL_FROM_NAME` (see `WebAuthSettings::from_env`). That gate proves credentials
are *present* — it cannot prove mail is *delivered*. A misconfigured SPF/DKIM
record, a relay that silently drops, or a `MAIL_FROM_ADDRESS` the receiving MTA
rejects all pass boot and then break verification + recovery for real users.

Run this controlled-inbox smoke **before** you flip `WEB_AUTH_ENABLED=true` on
production. The controlled inbox is an address **you own** (e.g. a personal
mailbox or a throwaway you can read), never a real user.

### Procedure

1. **Keep production off.** On the production env class, leave `WEB_AUTH_ENABLED`
   unset/false. The boot gate below runs only under `RUST_ENV=production` +
   `WEB_AUTH_ENABLED=true`, so this smoke runs without touching it.

2. **Stand up the mail stack on a non-production instance.** Point a staging
   container (or a local `cargo run --bin ruxlog` with `RUST_ENV=development`)
   at the *same* mail config production will use:
   - `MAIL_PROVIDER=smtp` (or `cloudflare`) + the real credentials
     (`SMTP_HOST`/`SMTP_USERNAME`/`SMTP_PASSWORD`, or
     `CLOUDFLARE_EMAIL_ACCOUNT_ID`/`CLOUDFLARE_EMAIL_API_TOKEN`).
   - The real `MAIL_FROM_ADDRESS` + `MAIL_FROM_NAME`.
   - `WEB_AUTH_ENABLED=true` — under a non-production env class the prod boot
     gate is skipped, so the auth router mounts and the MailRouter runs the
     genuine code path (same templates, same from-address, same provider).

3. **Send a verification email to the controlled inbox.** Register/authenticate
   the controlled address through the web auth flow so the verification-code
   email fires, or hit the verification-send endpoint directly. This exercises
   the real `send_email_verification_code` → `MailRouter` → provider path.

4. **Confirm delivery at the controlled inbox:**
   - The message arrives within a reasonable window (a few minutes).
   - `From:` matches `MAIL_FROM_ADDRESS` / `MAIL_FROM_NAME`.
   - Authentication-Results show **SPF pass + DKIM pass** for your sending
     domain.
   - It is in the inbox, not the spam/junk folder.

5. **Only after delivery is confirmed**, set `WEB_AUTH_ENABLED=true` on the
   production env class and redeploy.

### Fast transport-only pre-check (optional)

Before the full smoke above, a quick relay check with `swaks` confirms the SMTP
transport + credentials + from-address are accepted end-to-end (it does **not**
exercise the api's MailRouter or templates, so it cannot replace step 3):

```bash
swaks --auth \
  --server "$(grep ^SMTP_HOST= .env | cut -d= -f2-):587" \
  --to controlled-inbox@example.com \
  --from "$(grep ^MAIL_FROM_ADDRESS= .env | cut -d= -f2-)" \
  --auth-user "$(grep ^SMTP_USERNAME= .env | cut -d= -f2-)" \
  --auth-password "$(grep ^SMTP_PASSWORD= .env | cut -d= -f2-)" \
  --header "Subject: EasyQuran mail smoke"
```

### If delivery fails

Do **not** enable `WEB_AUTH_ENABLED=true` on production. Check, in order: SMTP
credentials + host/port, the sending domain's SPF/DKIM/DMARC records, that
`MAIL_FROM_ADDRESS` is on a domain you control, and (for `MAIL_PROVIDER=cloudflare`)
that the recipient is in `CLOUDFLARE_EMAIL_ALLOWED_ADDRESSES` if you set that
allowlist. Re-run the smoke from step 2 until the controlled inbox receives the
mail with SPF+DKIM pass.

## Notes

- `/api` is stripped at the edge (`easyquran.fyi/api/quran/health/ready` → `/quran/health/ready`). The public api router excludes `/api/healthz` (`!PathPrefix(`/api/healthz`)`), so `/healthz` is reachable only on the Docker network / in-container healthcheck — never via the public host router.
- API reads Quran sources from a read-only bind mount (`./db/quran` → `/app/quran`,
  compose `volumes:`); the image no longer bakes them. `db/` is gitignored — the DBs live in
  R2 and must be provisioned into the checkout on the host before the api container starts. Immutability is enforced both by the
  `:ro` mount (FS-level read-only for current and future inodes — strictly stronger than the
  old `chmod -R a-w`) and by `read_only(true).immutable(true)` in the loader. The
  Dockerfile.api COPY removal and the compose mount are a coupled pair — ship and revert together.
- The web runtime reads `quran-data.json` from the build's `client/quran-meta/` output
  (adapter-node copies `static/` → `build/client/`).
- `web_quran_cache` is disposable derived HTML; removing it causes cold SSR only.
- The api binary is still named `ruxlog` (a ported backend) — cosmetic.
- VPS needs ≥2 GB RAM for the Rust build (add swap on smaller boxes).
- Env normalization: the production `.env` is intentionally minimal (5 required
  vars — see `deploy/.env.example`). Everything else is a co-location invariant
  hardcoded in `docker-compose.yml` (`RUST_ENV`, `IP_SOURCE`,
  `INTERNAL_QURAN_API_BASE`), an interpolated compose default overridable via
  `.env` (the `S3_*` dummies, `MAIL_*`, proxy topology, all `QURAN_*` tuning), or
  a code default. `S3_*` / `MAIL_PROVIDER` set in the Dokploy env tab override
  the compose dummies when media/auth are wired — no compose edit needed.
- `just docker-up prod` (root `.env`, no local override) boots the api with
  `RUST_ENV=production` and so fails closed unless the root `.env` carries real
  `COOKIE_KEY` / `FIELD_ENC_KEY` / `INTERNAL_QURAN_API_TOKEN` / `ALLOWED_ORIGINS`.
  Use the Dokploy flow for production; use `just docker-up local` for local dev.
