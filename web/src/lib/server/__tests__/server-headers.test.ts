import { createHash } from "node:crypto";
import type { ServerResponse } from "node:http";

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

type HeaderStore = Map<string, string>;

interface StubbedResponse {
  headers: HeaderStore;
  response: ServerResponse;
}

function stubResponse(initial?: readonly (readonly [string, string])[]): StubbedResponse {
  const headers: HeaderStore = new Map(
    (initial ?? []).map(([name, value]): [string, string] => [name.toLowerCase(), value]),
  );
  // SAFETY: applyHeaders only touches getHeader/setHeader on the response; the
  // literal implements exactly those two OutgoingMessage members, and the
  // assertion narrows it to that usage for the duration of the call.
  const response = {
    getHeader: (name: string): number | string | string[] | undefined =>
      headers.get(name.toLowerCase()),
    setHeader: (name: string, value: number | string | readonly string[]): void => {
      headers.set(name.toLowerCase(), String(value));
    },
  } as ServerResponse;
  return { headers, response };
}

describe("applyHeaders CSP stamping", () => {
  it("stamps the hash-bearing CSP on a 200 HTML response for a prerendered path", () => {
    const { headers, response } = stubResponse();
    applyHeaders(response, "/", 200, { "Content-Type": "text/html; charset=utf-8" }, PAGE_HASHES);
    const csp = headers.get("content-security-policy");
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain(hash(THEME_SCRIPT));
    expect(headers.get("x-content-type-options")).toBe("nosniff");
    expect(headers.get("strict-transport-security")).toBe(HSTS);
  });

  it("keeps the hashless static policy on a 200 HTML response without page hashes", () => {
    const { headers, response } = stubResponse();
    applyHeaders(
      response,
      "/this-page-has-no-hashes",
      200,
      { "Content-Type": "text/html; charset=utf-8" },
      PAGE_HASHES,
    );
    const csp = headers.get("content-security-policy");
    expect(csp).toContain("script-src 'self'");
    expect(csp?.includes("sha256-")).toBe(false);
  });

  it("never stamps a CSP on a 304 — the cached 200's policy stays authoritative", () => {
    // Regression: a hashless CSP on the 304 used to REPLACE the cached 200's
    // hash-bearing policy in the browser cache, CSP-blocking the page's inline
    // theme + hydration scripts after a service-worker revalidation.
    const { headers, response } = stubResponse();
    applyHeaders(response, "/", 304, { ETag: '"revalidate-me"' }, PAGE_HASHES);
    expect(headers.has("content-security-policy")).toBe(false);
    // The rest of the outer-pass contract still lands on the revalidation.
    expect(headers.get("x-content-type-options")).toBe("nosniff");
    expect(headers.get("strict-transport-security")).toBe(HSTS);
    expect(headers.get("cache-control")).toBe("no-cache");
  });

  it("never stamps a CSP on a 204 either", () => {
    const { headers, response } = stubResponse();
    applyHeaders(response, "/api/no-content", 204, undefined, PAGE_HASHES);
    expect(headers.has("content-security-policy")).toBe(false);
  });

  it("never stamps a CSP on a 200 without a content-type — hashes are unresolvable", () => {
    const { headers, response } = stubResponse();
    applyHeaders(response, "/", 200, undefined, PAGE_HASHES);
    expect(headers.has("content-security-policy")).toBe(false);
  });

  it("leaves a CSP set by an inner layer untouched", () => {
    const inner = "default-src 'self'; script-src 'self' 'nonce-inner-only'";
    const { headers, response } = stubResponse([
      ["Content-Security-Policy", inner],
      ["Content-Type", "text/html; charset=utf-8"],
    ]);
    applyHeaders(response, "/", 200, undefined, PAGE_HASHES);
    expect(headers.get("content-security-policy")).toBe(inner);
  });
});

describe("isMissingModule import guard", () => {
  it("treats Node missing-module rejections as the expected no-build condition", () => {
    const esm: NodeJS.ErrnoException = new Error(
      "Cannot find module './build/handler.js'",
    );
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
