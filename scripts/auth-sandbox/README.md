# Auth sandbox

Local fake OAuth provider for exercising the full login pipeline without real
GitHub/Discord credentials. The fake validates PKCE (S256) and one-time codes,
so a green run proves our state/CSRF + PKCE + token-exchange path works.

## Run

```bash
# 1. Fake provider (GitHub + Discord + Google on one port)
node scripts/auth-sandbox/oauth-provider.mjs --port 8090

# 2. API with sandbox overrides (development only; production ignores them).
#    Redirect URIs / allowed origins / WebAuthn RP derive from FRONTEND_URL —
#    unset, dev defaults to http://localhost:5173, so only the fake-provider
#    base URLs are needed here.
cd rust
set -a && . ../.env && set +a
APP_ENV=development WEB_AUTH_ENABLED=true WEB_OAUTH_PROVIDERS=github,discord,google \
GITHUB_CLIENT_ID=sandbox-gh-client GITHUB_CLIENT_SECRET=sandbox-gh-secret \
GITHUB_OAUTH_BASE_URL=http://127.0.0.1:8090 GITHUB_API_BASE_URL=http://127.0.0.1:8090 \
DISCORD_CLIENT_ID=sandbox-dc-client DISCORD_CLIENT_SECRET=sandbox-dc-secret \
DISCORD_OAUTH_BASE_URL=http://127.0.0.1:8090 DISCORD_API_BASE_URL=http://127.0.0.1:8090/api \
GOOGLE_CLIENT_ID=sandbox-google-client GOOGLE_CLIENT_SECRET=sandbox-google-secret \
GOOGLE_AUTH_BASE_URL=http://127.0.0.1:8090 GOOGLE_TOKEN_BASE_URL=http://127.0.0.1:8090 \
GOOGLE_JWKS_URL=http://127.0.0.1:8090/oauth2/v3/certs \
GOOGLE_USERINFO_URL=http://127.0.0.1:8090/oauth2/v2/userinfo \
cargo run -p ruxlog

# 3. Web dev server (separate shell)
just web-dev
```

Then sign in at `http://localhost:5173/login` (GitHub / Discord / Google buttons), or hit
`/api/auth/{provider}/v1/login` directly. Verify the session with
`GET /api/user/v1/get` — the sandbox identities are `sandbox-gh@example.com`
(GitHub), `sandbox-dc@example.com` (Discord) and `sandbox-google@example.com`
(Google).

## What it proves / doesn't

- Proves: login redirect, session-bound state, PKCE challenge/verifier, code
  exchange, profile fetch, account create/link, session rotation, frontend
  success exchange.
- Doesn't prove: the real providers' consent screens, scopes, or token issuance.
  Those need real OAuth apps (`GITHUB_*` / `DISCORD_*` credentials).
