import { createHash } from "node:crypto";

import { describe, expect, it } from "vite-plus/test";

import { variables } from "../../../env";
import {
  applyHeaders,
  buildEnvModule,
  DYNAMIC_PUBLIC_ENV_VARS,
  envModuleResponse,
  isMissingModule,
  isStaticMethod,
} from "../../../../server";

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

describe("/_app/env.js route", () => {
  // SAFETY: src/env.ts's `variables` is a literal-keyed mapped type; this test
  // only reads the public/static flags, so it is re-branded to that entry shape.
  const declaredDynamicPublic = Object.entries(
    variables as Record<string, { public?: boolean; static?: boolean }>,
  )
    .filter(([, config]) => config.public === true && config.static !== true)
    .map(([name]) => name);

  it("serves exactly the dynamic public vars declared in src/env.ts", () => {
    // kit 3 compiles an import of /_app/env.js into the built service worker
    // whenever such a var exists; adapter-bun never materializes the module,
    // so web/server.ts must — for exactly this set, or registration dies.
    expect([...DYNAMIC_PUBLIC_ENV_VARS].sort()).toEqual([...declaredDynamicPublic].sort());
  });

  it("serves a no-cache JS module read from process.env at request time", async () => {
    const previousEnv = process.env.PUBLIC_ENV;
    const previousKey = process.env.PUBLIC_FCM_VAPID_KEY;
    process.env.PUBLIC_ENV = "integration";
    delete process.env.PUBLIC_FCM_VAPID_KEY;
    try {
      const response = envModuleResponse(
        new Request("http://localhost/_app/env.js"),
        "/_app/env.js",
      );
      if (response === null) throw new Error("/_app/env.js route did not match");
      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toBe("application/javascript");
      // Never cached stale: a pinned copy would freeze a previous deploy's env.
      expect(response.headers.get("cache-control")).toBe("no-cache");
      // Same module shape kit's own builder.generateEnvModule() writes.
      const body = await response.text();
      expect(body.startsWith("export const env=")).toBe(true);
      // SAFETY: buildEnvModule() writes `export const env=` + JSON.stringify of a
      // string-valued record (shape test below), so JSON.parse returns exactly
      // Record<string, string>.
      const payload = JSON.parse(body.slice("export const env=".length)) as Record<string, string>;
      for (const name of DYNAMIC_PUBLIC_ENV_VARS) expect(name in payload).toBe(true);
      expect(payload.PUBLIC_ENV).toBe("integration");
      expect(payload.PUBLIC_FCM_VAPID_KEY).toBe("");
    } finally {
      process.env.PUBLIC_ENV = previousEnv;
      if (previousKey === undefined) delete process.env.PUBLIC_FCM_VAPID_KEY;
      else process.env.PUBLIC_FCM_VAPID_KEY = previousKey;
    }
  });

  it("escapes line terminators that are raw-forbidden in pre-ES2019 string literals", () => {
    const module = buildEnvModule(["PUBLIC_ENV"], { PUBLIC_ENV: "a\u2028b\u2029c" });
    expect(module).toBe('export const env={"PUBLIC_ENV":"a\\u2028b\\u2029c"}');
  });

  it("returns null off-path — nothing beyond the exact module path is intercepted", () => {
    expect(
      envModuleResponse(new Request("http://localhost/_app/env.js.map"), "/_app/env.js.map"),
    ).toBeNull();
    expect(envModuleResponse(new Request("http://localhost/"), "/")).toBeNull();
  });
});

describe("static layer method gate", () => {
  it("serves GET/HEAD only — other methods fall through to the kit handler", () => {
    // Regression: the static layer used to answer POST /al-fatihah with the
    // prerendered page body and 200. Non-GET/HEAD now reaches kit, which
    // answers 405 on SSR routes and 404 on prerendered-only paths (kit 3's
    // server instance excludes prerendered routes) — never the file body.
    expect(isStaticMethod("GET")).toBe(true);
    expect(isStaticMethod("HEAD")).toBe(true);
    for (const method of ["POST", "PUT", "DELETE", "PATCH", "OPTIONS"]) {
      expect(isStaticMethod(method)).toBe(false);
    }
    // The env module route applies the same gate: POST /_app/env.js falls through.
    expect(
      envModuleResponse(
        new Request("http://localhost/_app/env.js", { method: "POST" }),
        "/_app/env.js",
      ),
    ).toBeNull();
  });
});
