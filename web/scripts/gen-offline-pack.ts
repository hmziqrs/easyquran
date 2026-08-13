import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import type { Dirent } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(__dirname, "..");
const BUILD = path.join(WEB, "build");
const OFFLINE_DIR = path.join(BUILD, "client", "offline");
const MANIFEST_PATH = path.join(OFFLINE_DIR, "manifest.json");

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
    const parsed = JSON.parse(readFileSync(versionFile, "utf8")) as { version?: unknown };
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

const legacyReader = pairs.find(
  ({ key }) => key === "/app/__data.json" || key.startsWith("/app/"),
);
if (legacyReader) {
  throw new Error(`[offline] legacy unprefixed reader artifact leaked into build: ${legacyReader.key}`);
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
}

const manifest = {
  pack: `/offline/pack.${packId}.json`,
  bytes: Buffer.byteLength(serialized),
  entries: bodies.length,
  appVersion: packId,
};
writeFileSync(MANIFEST_PATH, JSON.stringify(manifest, null, 2) + "\n");

console.log(
  `[offline] pack ${packId} · ${bodies.length} entries · ${manifest.bytes} bytes → ${path.relative(WEB, packPath)}`,
);
