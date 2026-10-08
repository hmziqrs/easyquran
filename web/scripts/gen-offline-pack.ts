import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import type { Dirent } from "node:fs";
import { isBuiltin } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants as zlib, gzipSync } from "node:zlib";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(__dirname, "..");
const BUILD = path.join(WEB, "build");
const SERVER_DIR = path.join(BUILD, "server");
const OFFLINE_DIR = path.join(BUILD, "client", "offline");
const MANIFEST_PATH = path.join(OFFLINE_DIR, "manifest.json");

// adapter-bun externalizes package.json `dependencies` in the SSR build; the
// web image ships no node_modules, so vite.config.ts lists everything the
// server bundle may import under ssr.noExternal. If a bare specifier ever leaks
// into build/server as a real import, the container would crash at startup —
// fail the build here instead, naming the culprit.
//
// Static import/export statements stay line-anchored (rollup always emits them
// at line start) — including bare side-effect imports (`import "firebase/app";`),
// which carry no from-clause for the static regex to anchor on and so get
// their own line-anchored arm. Dynamic imports do NOT: the bundle carries
// them mid-line and await-prefixed (`fn(a, await import("firebase/analytics"))`),
// so `import(` is matched anywhere on the line, at EVERY occurrence — a
// relative dynamic import early on a line must not mask a later bare one.
// The lookbehind rejects member access
// (`foo.import("…")`) and identifier suffixes (`ximport("…")`); a string that
// merely CONTAINS `import("dependency-root")` can still match, but only
// production-dependency roots flag, and the error names the culprit for a
// human to confirm. Subpaths must reduce to their package root
// (`firebase/messaging` → `firebase`), scoped or not — package.json lists
// roots, never subpaths. Only production dependencies are flaggable — that is
// exactly the set the adapter externalizes.
function assertNoRuntimeExternals(): void {
  // SAFETY: package.json is repo-owned; the assertion spells exactly the one
  // field read, and Object.keys tolerates its absence below.
  const pkg = JSON.parse(readFileSync(path.join(WEB, "package.json"), "utf8")) as {
    dependencies?: Record<string, string>;
  };
  const dependencies = new Set(Object.keys(pkg.dependencies ?? {}));
  const importLine = /^(?:import|export)\s[^;]*?from\s*["']([^"']+)["']/u;
  const sideEffectImport = /^import\s*["']([^"']+)["']/u;
  const dynamicImport = /(?<![.\w$])import\s*\(\s*["']([^"']+)["']\s*\)/gu;
  const externals = new Set<string>();
  const flag = (specifier: string | undefined): void => {
    if (specifier === undefined || specifier === "") return;
    if (specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("#")) return;
    if (specifier.startsWith("node:") || isBuiltin(specifier)) return;
    const root = specifier
      .split("/")
      .slice(0, specifier.startsWith("@") ? 2 : 1)
      .join("/");
    if (dependencies.has(root)) externals.add(specifier);
  };
  const check = (line: string): void => {
    // At most one line-anchored import/export statement heads a rollup line
    // (from-clause or bare side-effect); dynamic imports can appear many
    // times mid-line, so every match's specifier is checked, not just the
    // first.
    flag((importLine.exec(line) ?? sideEffectImport.exec(line))?.[1]);
    for (const match of line.matchAll(dynamicImport)) flag(match[1]);
  };
  const walk = (directory: string): void => {
    if (!existsSync(directory)) return;
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".js")) {
        for (const line of readFileSync(full, "utf8").split("\n")) check(line);
      }
    }
  };
  walk(SERVER_DIR);
  if (externals.size > 0) {
    throw new Error(
      `[offline] server bundle keeps runtime-external imports (${[...externals].join(", ")}) — ` +
        "add them to ssr.noExternal in vite.config.ts or the web image cannot boot (no node_modules)",
    );
  }
}

assertNoRuntimeExternals();

function listDataFiles(root: string): string[] {
  const out: string[] = [];
  if (!existsSync(root)) return out;
  const stack: string[] = [root];
  while (stack.length > 0) {
    const current = stack.pop()!;
    let entries: Dirent<string>[];
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.name === "offline") continue;
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.name === "__data.json") out.push(full);
    }
  }
  return out;
}

function routeKey(file: string): string {
  const rel = path.relative(BUILD, file);
  const parts = rel.split(path.sep);
  if (parts[0] === "prerendered") parts.shift();
  return "/" + parts.join("/");
}

function readAppVersion(): string | null {
  const candidates = [path.join(BUILD, "client", "_app", "version.json")];
  const versionFile = candidates.find((c) => existsSync(c));
  if (!versionFile) return null;
  try {
    // SAFETY: version.json is the adapter's own build output; only the version field is read.
    const parsed = JSON.parse(readFileSync(versionFile, "utf8")) as { version?: unknown };
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- version is an untyped JSON.parse field; this check is the parse
    return typeof parsed.version === "string" ? parsed.version : null;
  } catch {
    return null;
  }
}

const pairs = listDataFiles(BUILD).map((file) => ({ file, key: routeKey(file) }));
pairs.sort((a, b) => {
  if (a.key < b.key) return -1;
  return a.key > b.key ? 1 : 0;
});

// Scheme A: the unprefixed shapes ARE the live shapes now (/page/13/…, /juz/1/…),
// so the legacy set inverts — the dead spellings are the localized /en|/ar app
// families plus the deleted /app hub key. Any of those in the build means the
// old scheme leaked back in.
const isLegacyReaderKey = (key: string): boolean =>
  key === "/app/__data.json" ||
  key === "/en/app/__data.json" ||
  key.startsWith("/en/app/") ||
  key === "/ar/app/__data.json" ||
  key.startsWith("/ar/app/");
const legacyReader = pairs.find(({ key }) => isLegacyReaderKey(key));
if (legacyReader) {
  throw new Error(
    `[offline] legacy pre-scheme-A reader artifact leaked into build: ${legacyReader.key}`,
  );
}

const entries: Record<string, number> = {};
const bodies: string[] = [];
for (const { file, key } of pairs) {
  if (key in entries) continue;
  entries[key] = bodies.length;
  bodies.push(readFileSync(file, "utf8"));
}

const serialized = JSON.stringify({ version: 1, entries, bodies });
const packId = readAppVersion();
if (!packId || !/^[A-Za-z0-9_-]+$/u.test(packId)) {
  throw new Error("[offline] SvelteKit build version is missing or unsafe");
}
const packPath = path.join(OFFLINE_DIR, `pack.${packId}.json`);

mkdirSync(OFFLINE_DIR, { recursive: true });

if (!existsSync(packPath)) {
  writeFileSync(packPath, serialized);
  // adapter-bun precompresses build/client during adapt(); this pack is written
  // after that pass and after the adapter froze its build-time asset table, so
  // without these siblings the raw ~14 MB body ships uncompressed on every
  // install. Same settings kit's own builder.compress uses: brotli max quality,
  // gzip 9. (web/server.ts serves /offline/* from disk with the same
  // br/gz negotiation as the adapter's own asset routes.)
  const raw = Buffer.from(serialized);
  writeFileSync(
    `${packPath}.br`,
    brotliCompressSync(raw, {
      params: {
        [zlib.BROTLI_PARAM_QUALITY]: zlib.BROTLI_MAX_QUALITY,
        [zlib.BROTLI_PARAM_SIZE_HINT]: raw.byteLength,
      },
    }),
  );
  writeFileSync(`${packPath}.gz`, gzipSync(raw, { level: zlib.Z_BEST_COMPRESSION }));
}

const manifest = {
  pack: `/offline/pack.${packId}.json`,
  bytes: Buffer.byteLength(serialized),
  entries: bodies.length,
  appVersion: packId,
};
writeFileSync(MANIFEST_PATH, JSON.stringify(manifest, null, 2) + "\n");

const wireBytes = existsSync(`${packPath}.br`) ? statSync(`${packPath}.br`).size : manifest.bytes;
console.log(
  `[offline] pack ${packId} · ${bodies.length} entries · ${manifest.bytes} bytes ` +
    `(${wireBytes} brotli) → ${path.relative(WEB, packPath)}`,
);
