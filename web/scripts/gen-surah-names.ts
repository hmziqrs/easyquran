/**
 * Bake the diacritized surah-name list into the web data layer.
 *
 * Upstream: Al Quran Cloud (Islamic Network) `GET /v1/surah` — the only live
 * dataset found that ships every surah name with full tashkeel ("سُورَةُ ٱلْفَاتِحَةِ").
 * Tanzil `quran-data.xml` (our metadata source) stores plain names, so this is
 * a separate source, mirrored verbatim under db/quran/surah-names/ (gitignored;
 * re-fetched when absent) and baked into the tracked web/src/lib/data/surah-names.json.
 *
 * The generator never invents or edits a single name: it mirrors upstream bytes,
 * then only validates that each name's base letters (marks/tatweel/alef-form
 * folded) match the plain Tanzil name already baked in web/static/quran-meta/quran-data.json.
 * Any mismatch fails the run instead of writing.
 *
 *   pnpm quran:names            # use local mirror if present
 *   pnpm quran:names --refresh  # re-fetch upstream, replace the mirror
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const WEB = path.resolve(scriptDir, "..");
const REPO = path.resolve(WEB, "..");

const MIRROR_DIR = path.join(REPO, "db", "quran", "surah-names");
const MIRROR_FILE = path.join(MIRROR_DIR, "surah.json");
const MIRROR_INDEX = path.join(MIRROR_DIR, "index.json");
const SNAPSHOT = path.join(WEB, "static", "quran-meta", "quran-data.json");
const OUT = path.join(WEB, "src", "lib", "data", "surah-names.json");

const ENDPOINT = "https://api.alquran.cloud/v1/surah";
const SOURCE = "Al Quran Cloud (Islamic Network)";
const LICENSE =
  "alquran.cloud terms of use: free reproduction and display with acknowledgement; non-commercial. https://alquran.cloud/terms-and-conditions";

const SURAH_COUNT = 114;

interface AlquranCloudSurah {
  number: number;
  name: string;
}

interface AlquranCloudPayload {
  code: number;
  status: string;
  data: AlquranCloudSurah[];
}

function fail(message: string): never {
  throw new Error(`[surah-names] ${message}`);
}

/** Base-letter form for cross-source comparison: no marks, tatweel, alef-form or space differences. */
function baseLetters(input: string): string {
  return input
    .replace(/\u0640/gu, "")
    .replace(/\p{M}/gu, "")
    .replace(/\u0671|\u0622|\u0623|\u0625/gu, "\u0627")
    .replace(/\u0649/gu, "\u064A")
    .replace(/\s+/gu, "");
}

function plainNames(): string[] {
  // SAFETY: quran-data.json is the repo's tracked positional snapshot; rows[1] is the surah array, checked immediately below.
  const snapshot = JSON.parse(readFileSync(SNAPSHOT, "utf8")) as unknown[];
  // SAFETY: same positional contract as the line above; Array.isArray + length are checked on the next line.
  const rows = snapshot[1] as unknown[][];
  if (!Array.isArray(rows) || rows.length !== SURAH_COUNT)
    fail("snapshot surah rows are malformed");
  return rows.map((row, index) => {
    const name = row[2];
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- positional snapshot field is untyped JSON; this is the sole read
    if (typeof name !== "string") fail(`snapshot surah ${index + 1}: plain name missing`);
    return name;
  });
}

async function fetchUpstream(): Promise<AlquranCloudPayload> {
  const response = await fetch(ENDPOINT, { headers: { accept: "application/json" } });
  if (!response.ok) fail(`upstream fetch failed: ${response.status}`);
  // SAFETY: upstream wire payload; validate() runs before the payload is written or baked.
  return (await response.json()) as AlquranCloudPayload;
}

function writeMirror(payload: AlquranCloudPayload): void {
  mkdirSync(MIRROR_DIR, { recursive: true });
  writeFileSync(MIRROR_FILE, JSON.stringify(payload, null, 2) + "\n");
  writeFileSync(
    MIRROR_INDEX,
    JSON.stringify(
      {
        source: SOURCE,
        sourceUrl: ENDPOINT,
        format: "json (REST /v1/surah payload, mirrored verbatim)",
        license: LICENSE,
        retrieved: new Date().toISOString().slice(0, 10),
        count: SURAH_COUNT,
        file: { mirror: path.basename(MIRROR_FILE) },
      },
      null,
      2,
    ) + "\n",
  );
}

function readMirror() {
  if (!existsSync(MIRROR_FILE) || !existsSync(MIRROR_INDEX))
    fail("mirror missing — run with --refresh");
  // SAFETY: mirror files are written only after validate() accepted the payload.
  const payload = JSON.parse(readFileSync(MIRROR_FILE, "utf8")) as AlquranCloudPayload;
  // SAFETY: index.json is our own mirror metadata, written by writeMirror; retrieved is checked below.
  const index = JSON.parse(readFileSync(MIRROR_INDEX, "utf8")) as { retrieved?: unknown };
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- mirror metadata field is untyped JSON at this read
  const retrieved = typeof index.retrieved === "string" ? index.retrieved : "unknown";
  return { payload, retrieved };
}

function validate(payload: AlquranCloudPayload): string[] {
  if (payload.code !== 200 || payload.status !== "OK") fail("upstream payload is not OK");
  if (!Array.isArray(payload.data) || payload.data.length !== SURAH_COUNT) {
    fail(`expected ${SURAH_COUNT} surahs, got ${payload.data?.length ?? 0}`);
  }
  const expected = plainNames();
  const names: string[] = [];
  for (let i = 0; i < SURAH_COUNT; i += 1) {
    const surah = payload.data[i];
    if (!surah) fail(`surah ${i + 1}: missing upstream row`);
    if (surah.number !== i + 1) fail(`surah ${i + 1}: upstream number ${surah.number}`);
    if (surah.name.trim() === "") fail(`surah ${i + 1}: empty upstream name`);
    const body = surah.name.replace(/^\u0633\u064F\u0648\u0631\u064E\u0629\u064F\s*/u, "");
    if (baseLetters(body) !== baseLetters(expected[i]!)) {
      fail(`surah ${i + 1}: upstream "${surah.name}" does not fold to snapshot "${expected[i]}"`);
    }
    names.push(surah.name);
  }
  return names;
}

async function main(): Promise<void> {
  const refresh = process.argv.includes("--refresh");
  if (refresh || !existsSync(MIRROR_FILE) || !existsSync(MIRROR_INDEX)) {
    const payload = await fetchUpstream();
    validate(payload);
    writeMirror(payload);
  }
  const { payload, retrieved } = readMirror();
  const names = validate(payload);

  writeFileSync(
    OUT,
    JSON.stringify(
      { source: SOURCE, sourceUrl: ENDPOINT, retrieved, license: LICENSE, names },
      null,
      2,
    ) + "\n",
  );
  console.log(`wrote ${OUT} (${names.length} names, retrieved ${retrieved})`);
}

await main();
