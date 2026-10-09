# Auth sandbox

Local fake OAuth provider for exercising the full login pipeline without real
GitHub/Discord credentials. The fake validates PKCE (S256) and one-time codes,
so a green run proves our state/CSRF + PKCE + token-exchange path works.

## Run

```bash
# 1. Fake provider (GitHub + Discord on one port)
node scripts/auth-sandbox/oauth-provider.mjs --port 8090

# 2. API with sandbox overrides (development only; production ignores them)
cd rust
set -a && . ../.env && set +a
RUST_ENV=development WEB_AUTH_ENABLED=true WEB_OAUTH_PROVIDERS=github,discord \
GITHUB_CLIENT_ID=sandbox-gh-client GITHUB_CLIENT_SECRET=sandbox-gh-secret \
GITHUB_REDIRECT_URI=http://localhost:5173/api/auth/github/v1/callback \
GITHUB_OAUTH_BASE_URL=http://127.0.0.1:8090 GITHUB_API_BASE_URL=http://127.0.0.1:8090 \
DISCORD_CLIENT_ID=sandbox-dc-client DISCORD_CLIENT_SECRET=sandbox-dc-secret \
DISCORD_REDIRECT_URI=http://localhost:5173/api/auth/discord/v1/callback \
DISCORD_OAUTH_BASE_URL=http://127.0.0.1:8090 DISCORD_API_BASE_URL=http://127.0.0.1:8090/api \
FRONTEND_URL=http://localhost:5173 OAUTH_ALLOWED_REDIRECT_ORIGINS=http://localhost:5173 \
cargo run -p ruxlog

# 3. Web dev server (separate shell)
just web-dev
```

Then sign in at `http://localhost:5173/login` (GitHub / Discord buttons), or hit
`/api/auth/{provider}/v1/login` directly. Verify the session with
`GET /api/user/v1/get` — the sandbox identities are `sandbox-gh@example.com`
(GitHub) and `sandbox-dc@example.com` (Discord).

## What it proves / doesn't

- Proves: login redirect, session-bound state, PKCE challenge/verifier, code
  exchange, profile fetch, account create/link, session rotation, frontend
  success exchange.
- Doesn't prove: the real providers' consent screens, scopes, or token issuance.
  Those need real OAuth apps (`GITHUB_*` / `DISCORD_*` credentials).
