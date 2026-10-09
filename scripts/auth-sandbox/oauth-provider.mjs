#!/usr/bin/env node
/**
 * Local OAuth sandbox provider for `scripts/auth-sandbox` flows.
 *
 * Serves BOTH fake providers on one port:
 *   GitHub  — /login/oauth/authorize, /login/oauth/access_token, /user, /user/emails
 *   Discord — /oauth2/authorize, /api/oauth2/token, /api/users/@me, /api/oauth2/@me
 *
 * It validates PKCE (S256) for real, one-time codes, and bearer tokens — so a
 * green login run proves our OAuth pipeline (state/CSRF + PKCE + exchange)
 * actually works, not just that the fake replies 200.
 *
 * Usage:
 *   node scripts/auth-sandbox/oauth-provider.mjs [--port 8090]
 *
 * Point the API at it (development only):
 *   GITHUB_OAUTH_BASE_URL=http://127.0.0.1:8090 GITHUB_API_BASE_URL=http://127.0.0.1:8090
 *   DISCORD_OAUTH_BASE_URL=http://127.0.0.1:8090 DISCORD_API_BASE_URL=http://127.0.0.1:8090/api
 */

import { createHash, randomUUID } from "node:crypto";
import http from "node:http";

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const PORT = portArg >= 0 ? Number(args[portArg + 1]) : 8090;

/** code -> { provider, clientId, redirectUri, codeChallenge, scope } */
const pendingCodes = new Map();
/** access token -> { provider, clientId, userId } */
const accessTokens = new Map();

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

function issueCode(query, provider) {
  const code = `sandbox-code-${randomUUID()}`;
  pendingCodes.set(code, {
    provider,
    clientId: query.get("client_id") ?? "",
    redirectUri: query.get("redirect_uri") ?? "",
    codeChallenge: query.get("code_challenge") ?? "",
    scope: query.get("scope") ?? "",
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
  const userId = provider === "github" ? String(GITHUB_USER.id) : DISCORD_USER.id;
  accessTokens.set(token, { provider, clientId: record.clientId, userId });
  log(provider, "token issued", { scope: record.scope });

  const payload = {
    access_token: token,
    token_type: "bearer",
    scope: record.scope || (provider === "github" ? "user:email" : "identify email"),
  };
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
  log(`listening on http://127.0.0.1:${PORT} (github + discord fake provider)`);
});
