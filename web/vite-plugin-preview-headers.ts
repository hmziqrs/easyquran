import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import type { Plugin } from "vite";

import {
  cspWithScriptTokens,
  pageCandidates,
  parseStaticCsp,
  scriptHashTokens,
} from "./src/lib/server/preview-headers";

const WEB_ROOT = path.dirname(fileURLToPath(import.meta.url));
const PAGES_DIR = path.join(WEB_ROOT, ".svelte-kit/output/prerendered/pages");
const CLIENT_HEADERS_FILE = path.join(WEB_ROOT, ".svelte-kit/output/client/_headers");
const IMMUTABLE = "public, max-age=31536000, immutable";
const HSTS = "max-age=31536000; includeSubDomains";
const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;

const pageTokens = new Map<string, string[]>();

function isFile(candidate: string): boolean {
  return existsSync(candidate) && statSync(candidate).isFile();
}

function decodePath(pathname: string): string {
  try {
    return decodeURIComponent(pathname);
  } catch {
    return pathname;
  }
}

function scriptTokensFor(pathname: string): string[] {
  const cached = pageTokens.get(pathname);
  if (cached) return cached;
  const tokens: string[] = [];
  for (const candidate of pageCandidates(decodePath(pathname))) {
    const file = path.join(PAGES_DIR, candidate);
    if (!file.startsWith(`${PAGES_DIR}${path.sep}`) || !isFile(file)) continue;
    tokens.push(...scriptHashTokens(readFileSync(file, "utf8")));
    break;
  }
  pageTokens.set(pathname, tokens);
  return tokens;
}

// SvelteKit's preview stack serves prerendered pages and client assets ahead of
// the SSR handler, so hooks' applyHeaders never runs for them (web/server.ts
// covers the same gap in production with its outer pass). This middleware fills
// the security set from the generated static/_headers policy plus per-page
// inline-script hashes; later writers (hooks CSP on SSR, sirv cache-control)
// still win where they set their own values.
export function previewSecurityHeaders(): Plugin {
  return {
    name: "easyquran:preview-security-headers",
    apply: "serve",
    configurePreviewServer(server) {
      let staticCsp: string | undefined;
      let loaded = false;
      server.middlewares.use((req, res, next) => {
        const pathname = new URL(req.url ?? "/", "http://localhost").pathname;
        res.setHeader("X-Content-Type-Options", "nosniff");
        res.setHeader("Referrer-Policy", "strict-origin-when-cross-origin");
        res.setHeader("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
        res.setHeader("Strict-Transport-Security", HSTS);
        if (!loaded) {
          loaded = true;
          try {
            staticCsp = parseStaticCsp(readFileSync(CLIENT_HEADERS_FILE, "utf8"));
          } catch {
            staticCsp = undefined;
          }
        }
        const csp = staticCsp;
        if (csp !== undefined) {
          res.setHeader(
            "Content-Security-Policy",
            cspWithScriptTokens(csp, scriptTokensFor(pathname)),
          );
        }
        const immutablePath =
          pathname.startsWith("/_app/immutable/") ||
          pathname.startsWith("/_quran/tanzil/") ||
          packPattern.test(pathname);
        res.setHeader("Cache-Control", immutablePath ? IMMUTABLE : "no-cache");
        if (pathname.endsWith(".md") || pathname.endsWith(".txt")) {
          res.setHeader("X-Robots-Tag", "noindex, follow");
        }
        next();
      });
    },
  };
}
