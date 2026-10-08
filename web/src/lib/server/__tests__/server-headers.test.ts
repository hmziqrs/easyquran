import { createHash } from "node:crypto";

import { describe, expect, it } from "vite-plus/test";

import { applyHeaders, isMissingModule } from "../../../../server";

const THEME_SCRIPT = `const theme = localStorage.getItem("theme");document.dir = "ltr";`;

const HSTS = "max-age=31536000; includeSubDomains";

// Mirrors the boot-time scanPageScriptHashes output for one prerendered page.
const PAGE_HASHES = new Map<string, string[]>([
  ["/", [hash(THEME_SCRIPT)]],
  ["/index.html", [hash(THEME_SCRIPT)]],
]);

function hash(scriptBody: string): string {
  return `'sha256-${createHash("sha256").update(scriptBody).digest("base64")}'`;
}

function pageResponse(
  status: number,
  headers: Record<string, string> = {},
  hashes: ReadonlyMap<string, readonly string[]> = PAGE_HASHES,
  pathname = "/",
): Response {
  const response = new Response(status === 204 || status === 304 ? null : "body", {
    status,
    headers,
  });
  applyHeaders(response, pathname, hashes);
  return response;
}

describe("applyHeaders CSP stamping", () => {
  it("stamps the hash-bearing CSP on a 200 HTML response for a prerendered path", () => {
    const response = pageResponse(200, { "Content-Type": "text/html; charset=utf-8" });
    const csp = response.headers.get("content-security-policy");
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain(hash(THEME_SCRIPT));
    expect(response.headers.get("x-content-type-options")).toBe("nosniff");
    expect(response.headers.get("strict-transport-security")).toBe(HSTS);
  });

  it("keeps the hashless static policy on a 200 HTML response without page hashes", () => {
    const response = pageResponse(
      200,
      { "Content-Type": "text/html; charset=utf-8" },
      PAGE_HASHES,
      "/this-page-has-no-hashes",
    );
    const csp = response.headers.get("content-security-policy");
    expect(csp).toContain("script-src 'self'");
    expect(csp?.includes("sha256-")).toBe(false);
  });

  it("never stamps a CSP on a 304 — the cached 200's policy stays authoritative", () => {
    // Regression: a hashless CSP on the 304 used to REPLACE the cached 200's
    // hash-bearing policy in the browser cache, CSP-blocking the page's inline
    // theme + hydration scripts after a service-worker revalidation.
    const response = pageResponse(304, { ETag: '"revalidate-me"' });
    expect(response.headers.has("content-security-policy")).toBe(false);
    // The rest of the outer-pass contract still lands on the revalidation.
    expect(response.headers.get("x-content-type-options")).toBe("nosniff");
    expect(response.headers.get("strict-transport-security")).toBe(HSTS);
    expect(response.headers.get("cache-control")).toBe("no-cache");
  });

  it("never stamps a CSP on a 204 either", () => {
    const response = pageResponse(204);
    expect(response.headers.has("content-security-policy")).toBe(false);
  });

  it("never stamps a CSP on a 200 without a content-type — hashes are unresolvable", () => {
    // A null body keeps the Response constructor from defaulting content-type
    // to text/plain — the outer pass must see a genuinely absent content-type.
    const response = new Response(null, { status: 200 });
    applyHeaders(response, "/", PAGE_HASHES);
    expect(response.headers.has("content-security-policy")).toBe(false);
  });

  it("leaves a CSP set by an inner layer untouched", () => {
    const inner = "default-src 'self'; script-src 'self' 'nonce-inner-only'";
    const response = pageResponse(200, {
      "Content-Security-Policy": inner,
      "Content-Type": "text/html; charset=utf-8",
    });
    expect(response.headers.get("content-security-policy")).toBe(inner);
  });
});

describe("versioned IndoPak font caching", () => {
  it.each(["woff2", "ttf"])(
    "keeps the v4 %s immutable on success and revalidation",
    (extension) => {
      for (const status of [200, 304]) {
        const response = pageResponse(
          status,
          {},
          PAGE_HASHES,
          `/fonts/indopak-reader-compat-v4.${extension}`,
        );
        expect(response.headers.get("cache-control")).toBe("public, max-age=31536000, immutable");
      }
    },
  );

  it("allows missing editions and unversioned fonts to be revalidated", () => {
    for (const [path, status] of [
      ["/fonts/indopak-reader-compat-v99.woff2", 404],
      ["/fonts/indopak-reader-compat.woff2", 200],
    ] as const) {
      const response = pageResponse(status, {}, PAGE_HASHES, path);
      expect(response.headers.get("cache-control")).toBe("no-cache");
    }
  });
});

describe("isMissingModule import guard", () => {
  it("treats Node missing-module rejections as the expected no-build condition", () => {
    const esm: NodeJS.ErrnoException = new Error("Cannot find module './build/adapter-bun.js'");
    esm.code = "ERR_MODULE_NOT_FOUND";
    const cjs: NodeJS.ErrnoException = new Error("Cannot find module 'handler'");
    cjs.code = "MODULE_NOT_FOUND";
    expect(isMissingModule(esm)).toBe(true);
    expect(isMissingModule(cjs)).toBe(true);
  });

  it("rethrows any other failure so a corrupt bundle crashes startup loudly", () => {
    // Truncated / corrupted build output surfaces as a parse error with no
    // `code` — swallowing it would bind a server that stalls every request
    // with zero log.
    expect(isMissingModule(new SyntaxError("Unexpected end of input"))).toBe(false);
    expect(isMissingModule(new Error("boom"))).toBe(false);
  });
});
