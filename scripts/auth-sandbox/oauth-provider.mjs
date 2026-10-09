#!/usr/bin/env node
/**
 * Local OAuth sandbox provider for `scripts/auth-sandbox` flows.
 *
 * Serves THREE fake providers on one port:
 *   GitHub  — /login/oauth/authorize, /login/oauth/access_token, /user, /user/emails
 *   Discord — /oauth2/authorize, /api/oauth2/token, /api/users/@me, /api/oauth2/@me
 *   Google  — /o/oauth2/v2/auth, /token (RS256 id_token), /oauth2/v3/certs, /oauth2/v2/userinfo
 *
 * It validates PKCE (S256) for real, one-time codes, bearer tokens, and signs
 * Google id_tokens against a per-boot RSA key served via JWKS — so a green login
 * run proves our OAuth pipeline (state/CSRF + PKCE + nonce + exchange) actually
 * works, not just that the fake replies 200.
 *
 * Usage:
 *   node scripts/auth-sandbox/oauth-provider.mjs [--port 8090]
 *
 * Point the API at it (development only):
 *   GITHUB_OAUTH_BASE_URL=http://127.0.0.1:8090 GITHUB_API_BASE_URL=http://127.0.0.1:8090
 *   DISCORD_OAUTH_BASE_URL=http://127.0.0.1:8090 DISCORD_API_BASE_URL=http://127.0.0.1:8090/api
 *   GOOGLE_AUTH_BASE_URL=http://127.0.0.1:8090 GOOGLE_TOKEN_BASE_URL=http://127.0.0.1:8090
 *   GOOGLE_JWKS_URL=http://127.0.0.1:8090/oauth2/v3/certs
 *   GOOGLE_USERINFO_URL=http://127.0.0.1:8090/oauth2/v2/userinfo
 */

import { createHash, createSign, generateKeyPairSync, randomUUID } from "node:crypto";
import http from "node:http";

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const PORT = portArg >= 0 ? Number(args[portArg + 1]) : 8090;

/** code -> { provider, clientId, redirectUri, codeChallenge, scope, nonce } */
const pendingCodes = new Map();
/** access token -> { provider, clientId, userId } */
const accessTokens = new Map();

const GOOGLE_ISSUER = "https://accounts.google.com";
const GOOGLE_KID = "sandbox-google-key-1";
// Per-boot RSA keypair: the JWKS endpoint serves the public half, id_tokens are
// RS256-signed with the private half, so the server-side JWKS verification runs
// for real against the sandbox.
const GOOGLE_KEY_PAIR = generateKeyPairSync("rsa", { modulusLength: 2048 });
const GOOGLE_PUBLIC_JWK = GOOGLE_KEY_PAIR.publicKey.export({ format: "jwk" });

function base64url(input) {
  return Buffer.from(input).toString("base64url");
}

function signGoogleIdToken(claims) {
  const header = { alg: "RS256", typ: "JWT", kid: GOOGLE_KID };
  const signingInput = `${base64url(JSON.stringify(header))}.${base64url(JSON.stringify(claims))}`;
  const signature = createSign("RSA-SHA256")
    .update(signingInput)
    .sign(GOOGLE_KEY_PAIR.privateKey, "base64url");
  return `${signingInput}.${signature}`;
}

const GITHUB_USER = {
  id: 1001,
  login: "sandbox-dev",
  name: "Sandbox Dev",
  email: "sandbox-gh-public@example.com",
  avatar_url: "http://127.0.0.1:8090/avatar/github.png",
};

const DISCORD_USER = {
  id: "100000000000000001",
  username: "sandbox-dev",
  global_name: "Sandbox Dev",
  email: "sandbox-dc@example.com",
  verified: true,
  avatar: "0a1b2c3d4e5f60718293a4b5c6d7e8f9",
};

const GOOGLE_USER = {
  id: "100000000000000002",
  email: "sandbox-google@example.com",
  verified_email: true,
  name: "Sandbox Google",
  picture: "http://127.0.0.1:8090/avatar/google.png",
};

function log(...parts) {
  console.log("[sandbox-provider]", ...parts);
}

function json(res, status, body) {
  const data = JSON.stringify(body);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(data),
  });
  res.end(data);
}

function redirect(res, location) {
  res.writeHead(302, { location });
  res.end();
}

function readForm(req) {
  return new Promise((resolve, reject) => {
    let body = "";
    req.on("data", (chunk) => {
      body += chunk;
      if (body.length > 64 * 1024) reject(new Error("form body too large"));
    });
    req.on("end", () => resolve(new URLSearchParams(body)));
    req.on("error", reject);
  });
}

function s256(verifier) {
  return createHash("sha256").update(verifier).digest("base64url");
}

function defaultScopeFor(provider) {
  if (provider === "github") return "user:email";
  if (provider === "discord") return "identify email";
  return "openid email profile";
}

function issueCode(query, provider) {
  const code = `sandbox-code-${randomUUID()}`;
  pendingCodes.set(code, {
    provider,
    clientId: query.get("client_id") ?? "",
    redirectUri: query.get("redirect_uri") ?? "",
    codeChallenge: query.get("code_challenge") ?? "",
    scope: query.get("scope") ?? "",
    nonce: query.get("nonce") ?? "",
  });
  return code;
}

function handleAuthorize(req, res, url, provider) {
  const query = url.searchParams;
  if (!query.get("client_id") || !query.get("redirect_uri")) {
    return json(res, 400, { error: "invalid_request", hint: "client_id + redirect_uri required" });
  }
  if (query.get("response_type") !== "code") {
    return json(res, 400, { error: "unsupported_response_type" });
  }
  const code = issueCode(query, provider);
  const location = new URL(query.get("redirect_uri"));
  location.searchParams.set("code", code);
  location.searchParams.set("state", query.get("state") ?? "");
  log(provider, "authorize → auto-approve", { clientId: query.get("client_id") });
  return redirect(res, location.toString());
}

async function handleToken(req, res, provider) {
  const form = await readForm(req);
  const record = pendingCodes.get(form.get("code") ?? "");
  if (!record || record.provider !== provider) {
    return json(res, 400, { error: "invalid_grant", error_description: "unknown or used code" });
  }
  pendingCodes.delete(form.get("code"));

  const verifier = form.get("code_verifier") ?? "";
  if (record.codeChallenge && s256(verifier) !== record.codeChallenge) {
    log(provider, "PKCE MISMATCH — login refused");
    return json(res, 400, {
      error: "invalid_grant",
      error_description: "PKCE verification failed",
    });
  }
  if (form.get("client_id") && form.get("client_id") !== record.clientId) {
    return json(res, 400, { error: "invalid_client" });
  }

  const token = `sandbox-${provider}-token-${randomUUID()}`;
  const userId =
    provider === "github"
      ? String(GITHUB_USER.id)
      : provider === "google"
        ? GOOGLE_USER.id
        : DISCORD_USER.id;
  accessTokens.set(token, { provider, clientId: record.clientId, userId });
  log(provider, "token issued", { scope: record.scope });

  const payload = {
    access_token: token,
    token_type: "bearer",
    scope: record.scope || defaultScopeFor(provider),
  };
  if (provider === "google") {
    const now = Math.floor(Date.now() / 1000);
    payload.id_token = signGoogleIdToken({
      iss: GOOGLE_ISSUER,
      azp: record.clientId,
      aud: record.clientId,
      sub: GOOGLE_USER.id,
      email: GOOGLE_USER.email,
      email_verified: true,
      name: GOOGLE_USER.name,
      picture: GOOGLE_USER.picture,
      nonce: record.nonce,
      iat: now,
      exp: now + 3600,
    });
  }
  const accept = String(req.headers.accept ?? "");
  if (accept.includes("application/json")) {
    return json(res, 200, payload);
  }
  const formBody = new URLSearchParams(payload).toString();
  res.writeHead(200, {
    "content-type": "application/x-www-form-urlencoded; charset=utf-8",
    "content-length": Buffer.byteLength(formBody),
  });
  return res.end(formBody);
}

function bearerToken(req) {
  const header = String(req.headers.authorization ?? "");
  return header.startsWith("Bearer ") ? header.slice(7) : "";
}

function requireToken(req, res, provider) {
  const token = bearerToken(req);
  const record = accessTokens.get(token);
  if (!record || record.provider !== provider) {
    json(res, 401, { message: "401: Unauthorized" });
    return null;
  }
  return record;
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url ?? "/", `http://127.0.0.1:${PORT}`);
  const { pathname } = url;

  void (async () => {
    try {
      if (req.method === "GET" && pathname === "/login/oauth/authorize") {
        return handleAuthorize(req, res, url, "github");
      }
      if (req.method === "POST" && pathname === "/login/oauth/access_token") {
        return await handleToken(req, res, "github");
      }
      if (req.method === "GET" && pathname === "/user") {
        if (!requireToken(req, res, "github")) return;
        return json(res, 200, GITHUB_USER);
      }
      if (req.method === "GET" && pathname === "/user/emails") {
        if (!requireToken(req, res, "github")) return;
        return json(res, 200, [
          { email: "sandbox-gh@example.com", primary: true, verified: true },
          { email: GITHUB_USER.email, primary: false, verified: true },
        ]);
      }
      if (req.method === "GET" && pathname === "/oauth2/authorize") {
        return handleAuthorize(req, res, url, "discord");
      }
      if (req.method === "GET" && pathname === "/o/oauth2/v2/auth") {
        return handleAuthorize(req, res, url, "google");
      }
      if (req.method === "POST" && pathname === "/token") {
        return await handleToken(req, res, "google");
      }
      if (req.method === "GET" && pathname === "/oauth2/v3/certs") {
        return json(res, 200, {
          keys: [{ ...GOOGLE_PUBLIC_JWK, kid: GOOGLE_KID, use: "sig", alg: "RS256" }],
        });
      }
      if (req.method === "GET" && pathname === "/oauth2/v2/userinfo") {
        if (!requireToken(req, res, "google")) return;
        return json(res, 200, GOOGLE_USER);
      }
      if (req.method === "POST" && pathname === "/api/oauth2/token") {
        return await handleToken(req, res, "discord");
      }
      if (req.method === "GET" && pathname === "/api/users/@me") {
        if (!requireToken(req, res, "discord")) return;
        return json(res, 200, DISCORD_USER);
      }
      if (req.method === "GET" && pathname === "/api/oauth2/@me") {
        const record = requireToken(req, res, "discord");
        if (!record) return;
        return json(res, 200, {
          application: { id: record.clientId, name: "EasyQuran (sandbox)" },
          scopes: ["identify", "email"],
          expires: new Date(Date.now() + 7 * 24 * 3600 * 1000).toISOString(),
          user: DISCORD_USER,
        });
      }
      return json(res, 404, { error: "not_found", path: pathname });
    } catch (error) {
      log("ERROR", String(error));
      return json(res, 500, { error: "sandbox_provider_error" });
    }
  })();
});

server.listen(PORT, "127.0.0.1", () => {
  log(`listening on http://127.0.0.1:${PORT} (github + discord + google fake provider)`);
});
