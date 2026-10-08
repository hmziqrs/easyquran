/// <reference types="node" />
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { join, resolve, sep } from "node:path";

import {
  isReaderMdPath,
  mdSiblingPathFor,
  negotiateMarkdownPath,
  notAcceptableBody,
  varyWithAccept,
} from "./src/lib/accept-parse";

// ---------------------------------------------------------------------------
// Minimal structural types for the Bun globals this entry uses. The web app's
// tsconfig deliberately does not include bun-types globally (its DOM/lib set is
// tuned for the browser-facing src tree), so the handful of APIs needed here are
// spelled locally instead. SAFETY: each member mirrors the Bun 1.4 documented
// surface; anything the adapter's own runtime uses that this file does not call
// is simply omitted.
interface BunFile extends Blob {
  readonly lastModified: number;
}

interface BunFetchServer {
  readonly url: string;
  readonly pendingRequests: number;
  requestIP(request: Request): { address: string } | null;
  timeout(request: Request, seconds: number): void;
  stop(force?: boolean): Promise<void>;
}

interface BunServeOptions {
  hostname?: string;
  port?: number;
  idleTimeout?: number;
  maxRequestBodySize?: number;
  development?: boolean;
  fetch: (request: Request, server: BunFetchServer) => Response | Promise<Response>;
}

interface BunGlobal {
  readonly version: string;
  readonly semver: { order(a: string, b: string): number };
  readonly env: Record<string, string | undefined>;
  file(path: string): BunFile;
  serve(options: BunServeOptions): BunFetchServer;
  CryptoHasher: new (algorithm: "blake2b256") => {
    update(data: Uint8Array): void;
    digest(encoding: "hex"): string;
  };
}

// SAFETY: `Bun` is a runtime global under bun and absent under Node/vitest; the
// ambient declaration spells exactly that optional shape without asserting over
// globalThis. All Bun access is gated on this lookup so importing the module for
// its header logic never needs bun.
declare global {
  var Bun: BunGlobal | undefined;
}

const bun = globalThis.Bun;

// adapter-bun's stock entry refuses to boot on a Bun older than the adapter's
// floor (node_modules/@sveltejs/adapter-bun/src/index.js:11-15); this custom
// entry replaced that file, so the same gate lives here — a stale local bun
// must fail loudly, not serve a build it cannot run. order() rather than a
// satisfies() range so canary builds (1.5.0-canary.1) still pass.
if (bun !== undefined && bun.semver.order(bun.version, "1.4.0") < 0) {
  throw new Error(`[server] requires Bun 1.4 or newer, but this is Bun ${bun.version}`);
}

// The kit Server instance re-exported by the adapter hand-off. SAFETY: the
// hand-off module is untyped build output; this interface spells the two
// members adapter-bun's own runtime calls.
interface KitServer {
  init(options: {
    env: Record<string, string | undefined>;
    read: (file: string) => ReadableStream<Uint8Array> | null;
  }): Promise<void>;
  respond(
    request: Request,
    options: {
      platform: { server: BunFetchServer };
      getClientAddress: () => string;
    },
  ): Promise<Response>;
}

type AssetKind = "client_asset" | "prerendered_page" | "prerendered_asset";

interface AssetMeta {
  readonly hash: string;
  readonly mtime: number;
  readonly br?: boolean;
  readonly gz?: boolean;
}

/** One entry of the adapter hand-off's flat `assets` tuple. */
type HandoffAsset = readonly [kind: AssetKind, url: string, file: string, meta: AssetMeta];

interface Handoff {
  readonly server: KitServer;
  readonly dir: string;
  readonly app_dir: string;
  readonly base: string;
  readonly origin: string | undefined;
  readonly assets: readonly HandoffAsset[];
  readonly redirects: readonly (readonly [source: string, status: number, location: string])[];
  readonly server_assets: readonly (readonly [file: string])[];
}

/** A static route this entry can serve straight from disk. */
interface StaticRoute {
  readonly kind: "file" | "redirect";
  readonly file?: string;
  readonly meta?: AssetMeta;
  readonly immutable?: boolean;
  readonly status?: number;
  readonly location?: string;
}

// A missing build/adapter-bun.js is the expected fresh-clone / unit-test
// condition. MODULE_NOT_FOUND is the CommonJS twin (defensive: the bundle never
// requires, but a corrupted one might). Bun's ResolveMessage carries the same
// ERR_MODULE_NOT_FOUND code and is an Error. Any other failure — corrupt
// output, a broken dependency of the bundle — must crash startup loudly like a
// static import would: a bound server whose handler is a no-op stalls every
// request with zero log, which is strictly worse than a crash.
export function isMissingModule(error: Error): boolean {
  if (!("code" in error)) return false;
  return error.code === "ERR_MODULE_NOT_FOUND" || error.code === "MODULE_NOT_FOUND";
}

// adapter-bun emits build/adapter-bun.js (the hand-off: the kit server, the
// output dir, and the build-time asset table) at build time, so it does not
// exist on a fresh clone — and the header logic below is unit-tested by
// importing this module without a build. Resolve the hand-off lazily through a
// non-literal specifier (keeps tsc from pulling untyped build output into the
// program); a missing build only leaves the module with no kit server, which
// nothing dispatches to outside production.
let handoff: Handoff | null = null;
try {
  const modulePath = "./build/adapter-bun.js";
  // SAFETY: build output is untyped; Handoff spells the adapter's emitted shape.
  handoff = (await import(modulePath)) as Handoff;
} catch (cause: unknown) {
  if (!(cause instanceof Error) || !isMissingModule(cause)) throw cause;
  // No build output on disk (fresh clone / unit-test import): handoff stays
  // null; requests would 503, but only `pnpm start` serves traffic.
}

const kitServer = handoff?.server ?? null;

const IMMUTABLE = "public, max-age=31536000, immutable";
const HSTS = "max-age=31536000; includeSubDomains";
const packPattern = /^\/offline\/pack\.[A-Za-z0-9_-]+\.json$/u;
const versionedIndopakFontPattern = /^\/fonts\/indopak-reader-compat-v[1-9]\d*\.(?:woff2|ttf)$/u;
const scriptPattern = /<script>([\s\S]*?)<\/script>/gu;

const prerenderedDir = handoff ? join(handoff.dir, "prerendered") : "build/prerendered";
const clientDir = handoff ? join(handoff.dir, "client") : "build/client";

// adapter-bun serves prerendered pages from a build-time asset table BEFORE
// hooks run, so their inline scripts (app.html theme script + kit's per-route
// bootstrap) carry no nonce. A per-request nonce would mean rewriting
// (pre)compressed bodies; the sanctioned fallback is a 'sha256-<hash>'
// script-src entry per served path, scanned once at boot from the prerendered
// output itself.
async function scanPageScriptHashes(): Promise<Map<string, string[]>> {
  const map = new Map<string, string[]>();
  const addFile = async (file: string): Promise<void> => {
    const html = await readFile(file, "utf8").catch(() => null);
    if (html === null) return;
    const hashes: string[] = [];
    for (const match of html.matchAll(scriptPattern)) {
      hashes.push(
        `'sha256-${createHash("sha256")
          .update(match[1] ?? "")
          .digest("base64")}'`,
      );
    }
    if (hashes.length === 0) return;
    const relative = file.slice(prerenderedDir.length).replace(/^\/+|\/+$/gu, "");
    const base = relative.replace(/\.html$/u, "");
    const keys = base === "index" ? ["/", "/index.html"] : [`/${base}`, `/${base}.html`];
    for (const key of keys) map.set(key, hashes);
  };
  const walk = async (directory: string): Promise<void> => {
    const entries = await readdir(directory, { withFileTypes: true }).catch(() => null);
    if (entries === null) return;
    for (const entry of entries) {
      const full = join(directory, entry.name);
      if (entry.isDirectory()) await walk(full);
      else if (entry.name.endsWith(".html")) await addFile(full);
    }
  };
  await walk(prerenderedDir);
  return map;
}

const pageScriptHashes = await scanPageScriptHashes();

function apiOrigin(): string {
  const base = (process.env.PUBLIC_QURAN_API_BASE ?? "").replace(/\/+$/, "");
  return base ? `${base}/` : "";
}

function buildCsp(scriptTokens: readonly string[]): string {
  const connectSrc = [
    "'self'",
    "https://*.firebaseio.com",
    "wss://*.firebaseio.com",
    "https://firestore.googleapis.com",
    "https://firebase.googleapis.com",
    "https://firebaseinstallations.googleapis.com",
    "https://firebaseremoteconfig.googleapis.com",
    "https://firebaselogging.googleapis.com",
    "https://firebaselogging-pa.googleapis.com",
    "https://fcmregistrations.googleapis.com",
    "https://play.google.com",
    "https://www.google-analytics.com",
    "https://www.google.com",
  ];
  const api = apiOrigin();
  if (api) connectSrc.push(api);
  const scriptSrc = [
    "'self'",
    "'wasm-unsafe-eval'",
    ...scriptTokens,
    "https://www.gstatic.com",
    "https://www.googletagmanager.com",
  ];
  return [
    "default-src 'self'",
    `script-src ${scriptSrc.join(" ")}`,
    `connect-src ${connectSrc.join(" ")}`,
    "worker-src 'self' blob:",
    "img-src 'self' data: https:",
    "style-src 'self' 'unsafe-inline'",
    "object-src 'none'",
    "base-uri 'self'",
    "frame-ancestors 'none'",
  ].join("; ");
}

// Appends a header token (Vary, Link) without duplicating it. The inner layers
// (hooks) already set these on SSR responses; this merge covers prerendered and
// static responses, and SSR responses that reach the outer pass unmerged.
function appendHeaderToken(response: Response, name: string, token: string): void {
  const existing = response.headers.get(name);
  if (existing !== null && existing.includes(token)) return;
  response.headers.append(name, token);
}

// 1xx, 204, and 304 never carry a body — and the browser MERGES their headers
// into the cached entry they revalidate (prerendered HTML is `cache-control:
// no-cache`, so every navigation revalidates). Those responses carry no
// content-type, so the script hashes for the page cannot be resolved; a CSP
// stamped there would be the hashless fallback, REPLACING the cached 200's
// hash-bearing policy and CSP-blocking the page's theme + hydration inline
// scripts after a service-worker revalidation (JS-dead tab). Omit the CSP
// entirely instead: a 304 without a CSP leaves the cached 200's policy in force.
function hasBody(statusCode: number): boolean {
  if (statusCode < 200) return false;
  if (statusCode === 204 || statusCode === 304) return false;
  return true;
}

export function applyHeaders(
  response: Response,
  pathname: string,
  pageHashes: ReadonlyMap<string, readonly string[]> = pageScriptHashes,
): void {
  const setIfAbsent = (name: string, value: string): void => {
    if (response.headers.get(name) === null) response.headers.set(name, value);
  };
  setIfAbsent("X-Content-Type-Options", "nosniff");
  setIfAbsent("Referrer-Policy", "strict-origin-when-cross-origin");
  setIfAbsent("Permissions-Policy", "camera=(), microphone=(), geolocation=()");
  setIfAbsent("Strict-Transport-Security", HSTS);
  const contentType = response.headers.get("content-type");
  const isHtml = contentType !== null && contentType.includes("text/html");
  const cspMissing = response.headers.get("content-security-policy") === null;
  if (hasBody(response.status) && contentType !== null && cspMissing) {
    const hashes = isHtml ? pageHashes.get(pathname) : undefined;
    response.headers.set("Content-Security-Policy", buildCsp(hashes ?? []));
  }
  const mdPath = mdSiblingPathFor(pathname);
  if (isHtml && mdPath !== null) {
    // Hooks already appends the same token on SSR HTML; prerendered HTML has no
    // Link yet. Merge without duplicating either way.
    appendHeaderToken(response, "Link", `<${mdPath}>; rel="alternate"; type="text/markdown"`);
  }
  if (response.headers.get("cache-control") === null) {
    if (response.status >= 500) {
      response.headers.set("Cache-Control", "no-store");
    } else if (
      pathname.startsWith("/_app/immutable/") ||
      pathname.startsWith("/_quran/tanzil/") ||
      packPattern.test(pathname) ||
      ((response.status === 200 || response.status === 304) &&
        versionedIndopakFontPattern.test(pathname))
    ) {
      response.headers.set("Cache-Control", IMMUTABLE);
    } else {
      response.headers.set("Cache-Control", "no-cache");
    }
  }
  if (pathname.endsWith(".md") || pathname.endsWith(".txt")) {
    setIfAbsent("X-Robots-Tag", "noindex, follow");
  }
  if (
    mdPath !== null ||
    pathname.endsWith(".md") ||
    pathname.endsWith(".txt") ||
    response.status === 404
  ) {
    response.headers.set("Vary", varyWithAccept(response.headers.get("vary")));
  }
}

// ---------------------------------------------------------------------------
// Static serving. adapter-bun registers its asset table as native Bun.serve
// routes, which no wrapper can intercept — and the delivery contract must stamp
// headers on prerendered HTML and immutable assets too. This entry therefore
// serves the same table itself from the fetch layer. The ETag / 304 /
// brotli-gzip negotiation semantics below are a faithful port of
// @sveltejs/adapter-bun's src/routes-util.js (blake2b256 hashes are computed by
// the adapter at build time and carried in the hand-off; nothing here hashes
// file contents at request time except the postbuild-artifact fallback).
const CONTENT_ENCODING = { br: "br", gz: "gzip" } as const;

/** Late artifacts (offline pack, manifest) are written after the adapter ran. */
const lateMetaCache = new Map<string, AssetMeta>();

function isDotfile(url: string): boolean {
  return url
    .split("/")
    .some(
      (segment, index) => segment.startsWith(".") && !(index === 0 && segment === ".well-known"),
    );
}

function isFresh(request: Request, etag: string, mtime: number): boolean {
  const header = request.headers.get("if-none-match");
  if (header !== null) {
    return header
      .split(",")
      .some((value) => ["*", etag].includes(value.trim().replace(/^W\//u, "")));
  }
  const since = Date.parse(request.headers.get("if-modified-since") ?? "");
  return Number.isFinite(since) && Math.trunc(mtime / 1000) <= Math.trunc(since / 1000);
}

function negotiateEncoding(accept: string | null, meta: AssetMeta): "br" | "gz" | null {
  if (accept === null || (!meta.br && !meta.gz)) return null;
  const accepted = new Set<string>();
  for (const part of accept.split(",")) {
    const pieces = part.trim().toLowerCase().split(";");
    const params = pieces.slice(1).map((param) => param.trim());
    if (params.some((param) => /^q=0(\.0*)?$/u.test(param))) continue;
    accepted.add((pieces[0] ?? "").trim());
  }
  if (meta.br && (accepted.has("br") || accepted.has("*"))) return "br";
  if (meta.gz && (accepted.has("gzip") || accepted.has("*"))) return "gz";
  return null;
}

function staticRouteMap(h: Handoff): Map<string, StaticRoute> {
  const map = new Map<string, StaticRoute>();
  const put = (key: string, route: StaticRoute): void => {
    // First generated entry for a path wins, like sirv's lookup order —
    // exact files beat aliases. Skip a key already present.
    if (!map.has(key)) map.set(key, route);
  };
  for (const [kind, url, file, meta] of h.assets) {
    const onDisk = join(h.dir, kind === "client_asset" ? "client" : "prerendered", file);
    if (kind === "client_asset") {
      const immutable = url.startsWith(`${h.app_dir}/immutable/`);
      const route: StaticRoute = { kind: "file", file: onDisk, meta, immutable };
      put(`/${url}`, route);
      if (url.endsWith("/index.html") || url === "index.html") {
        const directory = `/${url.slice(0, -"index.html".length)}`;
        put(directory, route);
        if (directory !== "/") put(directory.replace(/\/$/u, ""), route);
      } else if (url.endsWith(".html")) {
        // sirv also serves `page.html` at `/page`
        put(`/${url.slice(0, -".html".length)}`, route);
      }
    } else if (kind === "prerendered_page") {
      put(url, { kind: "file", file: onDisk, meta });
      // The trailing-slash-inverted spelling 308s to the canonical URL.
      const inverted = url.endsWith("/") ? url.slice(0, -1) : `${url}/`;
      if (inverted && inverted !== "/") {
        put(inverted, { kind: "redirect", status: 308, location: url });
      }
    } else {
      put(`/${file}`, { kind: "file", file: onDisk, meta });
    }
  }
  for (const [source, status, location] of h.redirects) {
    put(source, { kind: "redirect", status, location });
  }
  return map;
}

const staticRoutes = handoff ? staticRouteMap(handoff) : new Map<string, StaticRoute>();

// The URL Standard's path percent-encode set, plus `%` and `\` so they stay
// literal — the canonical Location spelling for the 308 canonicalization.
// eslint-disable-next-line no-control-regex -- control characters are part of the URL path encode set
const ESCAPED_PATH_CHAR = /[\u0000-\u001f\u007f-\u{10ffff} "#<>?`{}%\\]/gu;

function encodePathname(pathname: string): string {
  return pathname.replace(ESCAPED_PATH_CHAR, (char) => encodeURIComponent(char));
}

async function lateAssetMeta(file: string): Promise<AssetMeta | null> {
  if (bun === undefined) return null;
  const cached = lateMetaCache.get(file);
  if (cached !== undefined) return cached;
  if (!existsSync(file)) return null;
  const hasher = new bun.CryptoHasher("blake2b256");
  const reader = bun.file(file).stream().getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done || value === undefined) break;
    hasher.update(value);
  }
  const meta: AssetMeta = {
    hash: hasher.digest("hex").slice(0, 16),
    mtime: bun.file(file).lastModified || 0,
    br: existsSync(`${file}.br`),
    gz: existsSync(`${file}.gz`),
  };
  lateMetaCache.set(file, meta);
  return meta;
}

// decodedPath is attacker-controlled (decodeURIComponent output): ../ survives
// URL parsing as %2f-encoded segments. Every candidate must resolve inside
// clientDir before it touches the filesystem. Dotfiles stay unserved, matching
// sirv's and adapter-bun's default.
function isSafeClientCandidate(candidate: string): boolean {
  if (candidate.includes("..")) return false;
  if (isDotfile(candidate.replace(/^\/+/u, ""))) return false;
  const root = resolve(clientDir);
  return resolve(root, `.${candidate}`).startsWith(`${root}${sep}`);
}

function serveFile(request: Request, file: string, meta: AssetMeta, immutable: boolean): Response {
  if (bun === undefined) throw new Error("[server] static file serving requires the bun runtime");
  // Bun serializes Range itself for file bodies; ranges apply to the identity
  // representation, so compressed variants are only negotiated without one.
  const encoding =
    request.headers.get("range") === null
      ? negotiateEncoding(request.headers.get("accept-encoding"), meta)
      : null;
  const etag = encoding === null ? `"${meta.hash}"` : `"${meta.hash}-${encoding}"`;
  const headers = new Headers({
    "content-type": bun.file(file).type,
    etag,
    "last-modified": new Date(meta.mtime).toUTCString(),
  });
  if (immutable) headers.set("cache-control", IMMUTABLE);
  if (meta.br || meta.gz) headers.set("vary", "accept-encoding");
  if (isFresh(request, etag, meta.mtime)) return new Response(null, { status: 304, headers });
  if (encoding !== null) {
    headers.set("content-encoding", CONTENT_ENCODING[encoding]);
    return new Response(bun.file(`${file}.${encoding}`), { status: 200, headers });
  }
  const body = request.method === "HEAD" ? null : bun.file(file);
  return new Response(body, { status: 200, headers });
}

// `decodedPath` (not the raw pathname) feeds applyHeaders so the CSP hash
// lookup matches the scanned page keys even for percent-encoded spellings.
function staticResponse(request: Request, key: string, decodedPath: string): Response | null {
  if (!isStaticMethod(request.method)) return null;
  const route = staticRoutes.get(key);
  if (route === undefined) return null;
  let response: Response | null = null;
  if (route.kind === "redirect" && route.status !== undefined && route.location !== undefined) {
    const search = new URL(request.url).search;
    response = new Response(null, {
      status: route.status,
      headers: { location: `${encodePathname(route.location)}${search}` },
    });
  } else if (route.kind === "file" && route.file !== undefined && route.meta !== undefined) {
    response = serveFile(request, route.file, route.meta, route.immutable === true);
  }
  if (response === null) return null;
  applyHeaders(response, decodedPath);
  return response;
}

// The static layer mirrors adapter-bun's GET-only native routes: serving file
// bodies for any other method would answer 200 with the page where the old
// stack let kit answer (405 on SSR routes; prerendered-only paths get kit's
// 404 — kit 3's server instance excludes prerendered routes). Every
// non-GET/HEAD request falls through to the kit handler instead.
export function isStaticMethod(method: string): boolean {
  return method === "GET" || method === "HEAD";
}

// Artifacts written after the adapter ran (the offline pack + its manifest,
// postbuild) are not in the hand-off table. Serve them from disk with the same
// freshness/encoding semantics, bounded to build/client.
async function lateStaticResponse(request: Request, pathname: string): Promise<Response | null> {
  if (bun === undefined || kitServer === null) return null;
  if (!isStaticMethod(request.method)) return null;
  if (!pathname.startsWith("/offline/")) return null;
  if (!isSafeClientCandidate(pathname)) return null;
  const file = join(clientDir, pathname);
  const meta = await lateAssetMeta(file);
  if (meta === null) return null;
  const response = serveFile(request, file, meta, packPattern.test(pathname));
  applyHeaders(response, pathname);
  return response;
}

// ---------------------------------------------------------------------------
// /_app/env.js — kit 3's runtime env module. With dynamic public vars declared
// in src/env.ts, kit compiles `import { env } from "/_app/env.js"` into the
// built client output (the service worker; pages receive their values through
// the SSR pass). adapter-node materializes that module from the running
// process; adapter-bun 1.0.0 never calls builder.generateEnvModule(), so this
// entry serves it instead — built per request from process.env, restricted to
// exactly the declared dynamic public set (unset reads as "", like the
// optionalString schema in src/env.ts). Deliberate sync: this list must mirror
// src/env.ts's `public: true` vars; server-headers.test.ts asserts the two
// stay in step (src/env.ts itself is NOT importable here — the web image ships
// no node_modules, so no @sveltejs/kit/env at runtime).
export const DYNAMIC_PUBLIC_ENV_VARS: readonly string[] = [
  "PUBLIC_API_BASE_URL",
  "PUBLIC_QURAN_API_BASE",
  "PUBLIC_ENV",
  "PUBLIC_FCM_VAPID_KEY",
];

// Same module shape kit's own builder.generateEnvModule() writes —
// `export const env={…}` (node_modules/@sveltejs/kit/src/core/adapt/builder.js).
// JSON.stringify output is a valid JS object literal for string values;
// U+2028/U+2029 are escaped because raw they predate ES2019 string literals.
export function buildEnvModule(
  names: readonly string[],
  source: Readonly<Record<string, string | undefined>> = process.env,
): string {
  const values: Record<string, string> = {};
  for (const name of names) values[name] = source[name] ?? "";
  const payload = JSON.stringify(values).replace(/[\u2028\u2029]/gu, (char) =>
    char === "\u2028" ? "\\u2028" : "\\u2029",
  );
  return `export const env=${payload}`;
}

function envModulePath(): string {
  if (handoff === null) return "/_app/env.js";
  // kit normalizes an empty base to "/" (build/adapter-bun.js: `base = "/"`);
  // strip the trailing slash so the joined path keeps a single leading one —
  // the exact spelling the built service worker imports.
  return `${handoff.base.replace(/\/+$/u, "")}/${handoff.app_dir}/env.js`;
}

// Served before the static table and the kit handler (adapter-node semantics:
// a runtime module, never a build artifact), and through the same
// applyHeaders security pass as every other response.
export function envModuleResponse(request: Request, pathname: string): Response | null {
  if (!isStaticMethod(request.method)) return null;
  if (pathname !== envModulePath()) return null;
  const body = request.method === "HEAD" ? null : buildEnvModule(DYNAMIC_PUBLIC_ENV_VARS);
  const response = new Response(body, {
    headers: {
      "content-type": "application/javascript",
      // never cached stale: a pinned copy would freeze a previous deploy's env
      "cache-control": "no-cache",
    },
  });
  applyHeaders(response, pathname);
  return response;
}

// ---------------------------------------------------------------------------
// Origin / client-address handling — ported from adapter-bun's src/handler.js
// so kit sees the public origin (CSRF + absolute URLs) exactly as the stock
// server would.
const protocolHeader = (process.env.PROTOCOL_HEADER ?? "").toLowerCase();
const hostHeader = (process.env.HOST_HEADER ?? "").toLowerCase();
const portHeader = (process.env.PORT_HEADER ?? "").toLowerCase();
const addressHeader = (process.env.ADDRESS_HEADER ?? "").toLowerCase();
const xffDepth = Number(process.env.XFF_DEPTH ?? 1);

function getOrigin(request: Request, url: URL): string {
  // assume TLS terminates upstream, like adapter-node/bun: an http origin would
  // fail CSRF checks.
  const protocol = decodeURIComponent(
    (protocolHeader !== "" && request.headers.get(protocolHeader)) || "https",
  );
  if (!/^https?$/iu.test(protocol)) {
    throw new Error(
      `The ${protocolHeader} header specified ${protocol} which is an invalid protocol scheme.`,
    );
  }
  const host =
    (hostHeader !== "" && request.headers.get(hostHeader)) ||
    (request.headers.get("host") ?? url.host);
  if (host === null || host === "") {
    throw new Error(`Could not determine host from the ${hostHeader || "host"} header`);
  }
  const port = portHeader !== "" ? request.headers.get(portHeader) : null;
  if (port !== null && Number.isNaN(Number(port))) {
    throw new Error(`The ${portHeader} header specified ${port} which is not a port number.`);
  }
  // canonicalized so the comparison with url.origin matches (case, default ports)
  return new URL(`${protocol}://${host}${port !== null ? `:${port}` : ""}`).origin;
}

function normalizeRequest(request: Request): Request | null {
  try {
    // an empty Host header makes request.url relative, so parsing belongs in the try
    const url = new URL(request.url);
    const requestOrigin = handoff?.origin ?? getOrigin(request, url);
    if (requestOrigin === url.origin) return request;
    return new Request(`${requestOrigin}${url.pathname}${url.search}`, request);
  } catch (cause: unknown) {
    console.error(
      `[server] could not determine request origin: ${cause instanceof Error ? cause.message : String(cause)}`,
    );
    return null;
  }
}

function getClientAddress(request: Request, server: BunFetchServer): string {
  if (addressHeader === "") {
    // requestIP() is null over unix sockets; undefined matches adapter-node
    return server.requestIP(request)?.address ?? "";
  }
  const value = request.headers.get(addressHeader);
  if (value === null) {
    throw new Error(
      `Address header was specified with ADDRESS_HEADER=${addressHeader} but is absent from request`,
    );
  }
  if (addressHeader !== "x-forwarded-for") return value;
  const addresses = value.split(",");
  if (xffDepth > addresses.length) {
    throw new Error(`XFF_DEPTH is ${xffDepth}, but only found ${addresses.length} addresses`);
  }
  return (addresses[addresses.length - xffDepth] ?? "").trim();
}

// ---------------------------------------------------------------------------
// Markdown negotiation over the prerendered output (the adapter serves those
// pages before hooks run, so Accept negotiation lives one layer above kit).
// decodedPath is attacker-controlled: every candidate must round-trip through
// the negotiated md-path patterns AND resolve inside prerenderedDir.
function isSafeMdCandidate(candidate: string): boolean {
  if (mdSiblingPathFor(candidate.replace(/\.md$/u, "")) !== candidate) return false;
  const root = resolve(prerenderedDir);
  return resolve(root, `.${candidate}`).startsWith(`${root}${sep}`);
}

// Localized reader .md spellings have no prerendered file of their own (/ar is
// never crawled; the content is locale-independent), so a de-localized
// prerendered file is tried first.
async function servePrerenderedMd(pathname: string, mdPath: string): Promise<Response | null> {
  const prefix = /^\/ar(?=\/)/u.exec(mdPath)?.[0];
  const candidates = prefix ? [mdPath, mdPath.slice(prefix.length)] : [mdPath];
  for (const candidate of candidates) {
    if (!isSafeMdCandidate(candidate)) continue;
    const body = await readFile(join(prerenderedDir, candidate)).catch(() => null);
    if (body !== null) {
      const response = new Response(body, {
        headers: { "content-type": "text/markdown; charset=utf-8" },
      });
      applyHeaders(response, pathname);
      return response;
    }
  }
  return null;
}

async function routeNegotiated(request: Request, decodedPath: string): Promise<Response | null> {
  if (!isStaticMethod(request.method)) return null;
  if (isReaderMdPath(decodedPath)) {
    const md = await servePrerenderedMd(decodedPath, decodedPath);
    if (md !== null) return md;
  }
  const negotiation = negotiateMarkdownPath(decodedPath, request.headers.get("accept"));
  if (negotiation.kind === "not-acceptable") {
    const response = new Response(notAcceptableBody(negotiation.accept), {
      status: 406,
      headers: {
        "content-type": "text/plain; charset=utf-8",
        "cache-control": "no-store",
        vary: "Accept",
      },
    });
    applyHeaders(response, decodedPath);
    return response;
  }
  if (negotiation.kind === "markdown") {
    const md = await servePrerenderedMd(decodedPath, negotiation.mdPath);
    if (md !== null) return md;
  }
  return null;
}

// ---------------------------------------------------------------------------
// The fetch pipeline: env module → markdown negotiation → static asset table → kit SSR.
async function handleRequest(request: Request, server: BunFetchServer): Promise<Response> {
  const url = new URL(request.url);
  let decodedPath = url.pathname;
  try {
    decodedPath = decodeURIComponent(url.pathname);
  } catch {
    // keep the raw path for lookups
  }

  // kit 3's dynamic-public env module, served from the running process before
  // any static/SSR layer can 404 it — the built service worker imports it.
  const envModule = envModuleResponse(request, decodedPath);
  if (envModule !== null) return envModule;

  const negotiated = await routeNegotiated(request, decodedPath);
  if (negotiated !== null) return negotiated;

  const staticResult =
    staticResponse(request, url.pathname, decodedPath) ??
    staticResponse(request, decodedPath, decodedPath);
  if (staticResult !== null) return staticResult;

  const late = await lateStaticResponse(request, decodedPath);
  if (late !== null) return late;

  if (kitServer === null) {
    const response = new Response("Service unavailable (no build output)", {
      status: 503,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
    applyHeaders(response, decodedPath);
    return response;
  }

  const normalized = normalizeRequest(request);
  if (normalized === null) {
    const response = new Response("Bad Request", {
      status: 400,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
    applyHeaders(response, decodedPath);
    return response;
  }

  let response: Response;
  try {
    response = await kitServer.respond(normalized, {
      platform: { server },
      getClientAddress: () => getClientAddress(request, server),
    });
  } catch (cause: unknown) {
    console.error("[server] unhandled request error", cause);
    response = new Response("Internal Server Error", {
      status: 500,
      // Declared as text/plain so the outer pass still stamps the security set
      // (CSP stamping is content-type-gated).
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  }

  // Event streams must not sit in the idle timeout or a buffering proxy.
  if ((response.headers.get("content-type") ?? "").startsWith("text/event-stream")) {
    server.timeout(request, 0);
    response.headers.set("x-accel-buffering", "no");
  }

  // The outermost header pass. Inner layers win: hooks set the full nonce-CSP
  // and privacy Cache-Control tiers on SSR responses — only missing headers are
  // filled in, and prerendered/static HTML gets the script-hash CSP.
  applyHeaders(response, decodedPath);
  return response;
}

// ---------------------------------------------------------------------------
// Boot. Unit tests import this module for its header logic; binding the port
// or initializing the built server inside the test runner would hold it open
// for the whole suite (and Node has no Bun runtime to init with).
const host = process.env.HOST ?? "0.0.0.0";
const port = Number(process.env.PORT ?? 3000);

if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error(`[server] invalid PORT: ${process.env.PORT ?? ""}`);
}

function bodySizeLimit(): number {
  const raw = process.env.BODY_SIZE_LIMIT;
  if (raw === undefined) return 512 * 1024;
  if (raw === "Infinity") return Infinity;
  const match = /^(\d+(?:\.\d*)?|\.\d+)([KMG])?$/iu.exec(raw);
  if (match === null) throw new Error(`[server] invalid BODY_SIZE_LIMIT: ${raw}`);
  return Number(match[1]) * bodySizeMultiplier(match[2]?.toUpperCase());
}

function bodySizeMultiplier(suffix: string | undefined): number {
  if (suffix === "K") return 1024;
  if (suffix === "M") return 1024 ** 2;
  if (suffix === "G") return 1024 ** 3;
  return 1;
}

function idleTimeout(): number {
  const raw = process.env.CONNECTION_IDLE_TIMEOUT;
  if (raw === undefined) return 30;
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value < 0 || value > 255) {
    throw new Error(`[server] invalid CONNECTION_IDLE_TIMEOUT: ${raw}`);
  }
  return value;
}

if (bun !== undefined && !process.env.VITEST && kitServer !== null) {
  const readFiles = new Map<string, string>();
  for (const [file] of handoff?.server_assets ?? []) readFiles.set(file, join(clientDir, file));
  await kitServer.init({
    env: bun.env,
    read: (file: string): ReadableStream<Uint8Array> | null => {
      const onDisk = readFiles.get(file);
      return onDisk === undefined ? null : bun.file(onDisk).stream();
    },
  });

  const server = bun.serve({
    hostname: host,
    port,
    idleTimeout: idleTimeout(),
    maxRequestBodySize: bodySizeLimit(),
    development: /^(1|true|yes|on)$/iu.test(process.env.DEVELOPMENT ?? ""),
    fetch: handleRequest,
  });

  console.log(`[server] listening on ${server.url}`);

  // Graceful shutdown: stop() waits for in-flight requests, and an open event
  // stream is one forever, so race the drain against a deadline. The timer must
  // stay referenced: a draining server no longer holds the event loop open.
  const shutdownTimeout = Number(process.env.SHUTDOWN_TIMEOUT ?? 30);
  let shuttingDown = false;
  const shutdown = (reason: string): void => {
    void (async () => {
      if (shuttingDown) process.exit(1);
      shuttingDown = true;
      if (server.pendingRequests !== 0) {
        console.log(
          `Waiting for ${server.pendingRequests} requests to finish before shutting down...`,
        );
      }
      let deadline: ReturnType<typeof setTimeout> | undefined;
      const drained = await Promise.race([
        server.stop().then(() => true),
        new Promise<boolean>((markDrained) => {
          deadline = setTimeout(() => markDrained(false), shutdownTimeout * 1000);
        }),
      ]);
      clearTimeout(deadline);
      // force-close aborts the in-flight handlers so shutdown listeners can
      // tear down resources those handlers still hold
      if (!drained) await server.stop(true);
      // SAFETY: kit listens for this custom event to close resources.
      process.emit("sveltekit:shutdown", reason);
    })();
  };
  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));
}
