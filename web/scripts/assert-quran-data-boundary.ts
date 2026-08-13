import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(scriptDir, "..");
const BUILD = path.join(WEB, "build");
const META = path.join(WEB, "static/quran-meta");
const SNAPSHOT_NAME = "quran-data.json";
const SNAPSHOT = path.join(META, SNAPSHOT_NAME);

function fail(message: string): never {
  throw new Error(`[quran-data-boundary] ${message}`);
}

function filesUnder(root: string, ignoredDirectories: ReadonlySet<string> = new Set()): string[] {
  if (!existsSync(root)) return [];
  const files: string[] = [];
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop()!;
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (!ignoredDirectories.has(entry.name)) stack.push(full);
      } else {
        files.push(full);
      }
    }
  }
  return files;
}

if (!existsSync(SNAPSHOT)) fail(`${SNAPSHOT_NAME} is required`);
const trackedSnapshots = filesUnder(META).filter((file) => file.endsWith(".json"));
if (trackedSnapshots.length !== 1 || trackedSnapshots[0] !== SNAPSHOT) {
  fail(`expected exactly one JSON snapshot named ${SNAPSHOT_NAME}`);
}
if (existsSync(path.join(META, "v1"))) fail("versioned data directories are forbidden");

const ignoredSourceDirectories = new Set([
  ".svelte-kit",
  ".vite",
  "baselines",
  "build",
  "node_modules",
  "static",
]);
const staticImports = filesUnder(WEB, ignoredSourceDirectories).filter((file) => {
  if (!/[.](?:cjs|js|mjs|svelte|ts)$/.test(file)) return false;
  const sourceText = readFileSync(file, "utf8");
  return /(?:\b(?:import|export)\s+(?:[^"'\n]+\s+from\s+)?["'][^"']*quran-data[.]json|\bimport\s*\(\s*["'][^"']*quran-data[.]json)/.test(
    sourceText,
  );
});
if (staticImports.length > 0) {
  fail(
    `snapshot has a static source import: ${staticImports.map((file) => path.relative(WEB, file)).join(", ")}`,
  );
}

// SAFETY: the snapshot is a positional array; its eight-field shape is checked immediately below.
const snapshot = JSON.parse(readFileSync(SNAPSHOT, "utf8")) as unknown[];
if (snapshot.length !== 8) fail("snapshot root must contain exactly eight positional fields");
// SAFETY: positional field 0 of the snapshot is the source array by format contract.
const source = snapshot[0] as unknown[];
const digest = source?.[0];
// eslint-disable-next-line anti-slop/no-runtime-typeof -- digest is an untyped JSON.parse field; this check is the boundary parse
if (typeof digest !== "string" || !/^[a-f0-9]{64}$/.test(digest)) {
  fail("source provenance digest is missing");
}

const textExtensions = new Set([".html", ".json", ".js", ".css", ".xml", ".txt"]);
const builtSnapshot = path.join(BUILD, `quran-meta/${SNAPSHOT_NAME}`);
if (!existsSync(builtSnapshot)) fail(`build/quran-meta/${SNAPSHOT_NAME} is missing`);
if (!readFileSync(builtSnapshot).equals(readFileSync(SNAPSHOT))) {
  fail(`build/quran-meta/${SNAPSHOT_NAME} differs from the tracked snapshot`);
}
const leaked = filesUnder(BUILD).filter((file) => {
  if (file === builtSnapshot) return false;
  if (!textExtensions.has(path.extname(file))) return false;
  return readFileSync(file, "utf8").includes(digest);
});
if (leaked.length > 0) {
  fail(
    `source snapshot leaked into build output: ${leaked.map((file) => path.relative(BUILD, file)).join(", ")}`,
  );
}

const serviceWorker = path.join(BUILD, "service-worker.js");
if (!existsSync(serviceWorker)) {
  fail("build/service-worker.js is missing; run the production build first");
}
if (readFileSync(serviceWorker, "utf8").includes(digest)) {
  fail(`snapshot provenance digest leaked into service-worker.js`);
}

for (const routeFile of ["app/al-fatihah.html", "app/al-fatihah/__data.json"]) {
  const routePath = path.join(BUILD, routeFile);
  if (!existsSync(routePath)) fail(`representative route output is missing: ${routeFile}`);
  if (readFileSync(routePath, "utf8").includes(SNAPSHOT_NAME)) {
    fail(`${SNAPSHOT_NAME} leaked into ${routeFile}`);
  }
}

console.log("Quran data boundary passed: one immutable snapshot, no build leak, no data embed.");
