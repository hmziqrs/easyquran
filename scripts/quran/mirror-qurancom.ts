/**
 * Mirror quran.com script-level reference assets into web/static/qurancom/.
 *
 * Modes:
 *   (default)   verify local mirror; fetch missing or size-mismatched assets
 *   --dry-run   report local state only; never writes, never fetches
 *   --record    fetch every asset and print id -> byte sizes to bake into the registry
 *
 * Identity is id + byte size; this script never hashes. It writes only under
 * web/static/qurancom/ (via .part + rename) and never touches db/.
 */

import { mkdir, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertAssetSize,
  assertMirrorTargetPath,
  mirrorAbsolutePath,
  QURANCOM_ASSETS,
  type QurancomAsset,
} from "./qurancom-assets.ts";

type MirrorMode = "verify" | "dry-run" | "record";

const FETCH_ATTEMPTS = 3;

function log(message: string): void {
  process.stdout.write(`${message}\n`);
}

function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

function parseMode(args: readonly string[]): MirrorMode {
  const supported = new Set(["--", "--dry-run", "--record"]);
  const unknown = args.filter((arg) => !supported.has(arg));
  if (unknown.length > 0) throw new Error(`unknown option(s): ${unknown.join(", ")}`);
  const dryRun = args.includes("--dry-run");
  const record = args.includes("--record");
  if (dryRun && record) throw new Error("--dry-run and --record are mutually exclusive");
  if (record) return "record";
  if (dryRun) return "dry-run";
  return "verify";
}

async function statSize(absolutePath: string): Promise<number | null> {
  try {
    const info = await stat(absolutePath);
    if (!info.isFile()) return null;
    return info.size;
  } catch {
    return null;
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

/** GET with two retries; transient DNS blips happened in recon, so a single failure is not final. */
async function download(url: string): Promise<Uint8Array> {
  let lastError = "unknown error";
  for (let attempt = 1; attempt <= FETCH_ATTEMPTS; attempt += 1) {
    try {
      const response = await fetch(url);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.byteLength === 0) throw new Error("empty response body");
      return bytes;
    } catch (error) {
      lastError = describeError(error);
      if (attempt < FETCH_ATTEMPTS) await delay(attempt * 1000);
    }
  }
  throw new Error(`fetch failed after ${FETCH_ATTEMPTS} attempts (${lastError}): ${url}`);
}

async function writeMirrorFile(asset: QurancomAsset, bytes: Uint8Array): Promise<number> {
  const absolute = mirrorAbsolutePath(asset);
  await mkdir(path.dirname(absolute), { recursive: true });
  const partPath = `${absolute}.part`;
  await writeFile(partPath, bytes);
  const partSize = await statSize(partPath);
  if (partSize !== bytes.byteLength) {
    await rm(partPath, { force: true });
    throw new Error(`${asset.id}: .part size ${partSize} != fetched ${bytes.byteLength}`);
  }
  await rename(partPath, absolute);
  const written = await statSize(absolute);
  if (written !== bytes.byteLength) {
    throw new Error(`${asset.id}: written size ${written} != fetched ${bytes.byteLength}`);
  }
  return written;
}

/**
 * Fetch one asset. When assertSizes is true the baked size is asserted before anything is
 * written (mismatched bytes never land) and re-verified from disk after the rename.
 */
async function fetchAsset(asset: QurancomAsset, assertSizes: boolean): Promise<number> {
  assertMirrorTargetPath(asset);
  const bytes = await download(asset.url);
  if (assertSizes) assertAssetSize(asset, bytes.byteLength);
  const written = await writeMirrorFile(asset, bytes);
  if (assertSizes) assertAssetSize(asset, written);
  return written;
}

async function fetchOrFail(asset: QurancomAsset, failures: string[]): Promise<void> {
  try {
    const size = await fetchAsset(asset, true);
    log(`${asset.id}: fetched and verified (${size} bytes)`);
  } catch (error) {
    failures.push(describeError(error));
    log(`${asset.id}: FAILED ${describeError(error)}`);
  }
}

async function runVerify(): Promise<readonly string[]> {
  const failures: string[] = [];
  for (const asset of QURANCOM_ASSETS) {
    assertMirrorTargetPath(asset);
    const localSize = await statSize(mirrorAbsolutePath(asset));
    if (localSize === null) {
      log(`${asset.id}: missing, fetching`);
      await fetchOrFail(asset, failures);
      continue;
    }
    if (asset.sizeBytes > 0 && localSize !== asset.sizeBytes) {
      log(`${asset.id}: local ${localSize} != recorded ${asset.sizeBytes}, refetching`);
      await fetchOrFail(asset, failures);
      continue;
    }
    log(`${asset.id}: ok (${localSize} bytes)`);
  }
  return failures;
}

async function runDryRun(): Promise<readonly string[]> {
  const failures: string[] = [];
  for (const asset of QURANCOM_ASSETS) {
    const localSize = await statSize(mirrorAbsolutePath(asset));
    if (localSize === null) {
      log(`${asset.id}: missing (would fetch)`);
      continue;
    }
    if (asset.sizeBytes > 0 && localSize !== asset.sizeBytes) {
      const mismatch = `${asset.id}: expected ${asset.sizeBytes}, actual ${localSize}`;
      failures.push(mismatch);
      log(`${mismatch} (would refetch)`);
      continue;
    }
    log(`${asset.id}: ok (${localSize} bytes)`);
  }
  return failures;
}

async function runRecord(): Promise<readonly string[]> {
  const failures: string[] = [];
  const recorded: Array<{ id: string; sizeBytes: number }> = [];
  for (const asset of QURANCOM_ASSETS) {
    try {
      const size = await fetchAsset(asset, false);
      recorded.push({ id: asset.id, sizeBytes: size });
      log(`${asset.id}: fetched (${size} bytes)`);
    } catch (error) {
      failures.push(describeError(error));
      log(`${asset.id}: FAILED ${describeError(error)}`);
    }
  }
  if (failures.length === 0) {
    const stamp = new Date().toISOString().slice(0, 10);
    log("");
    log(`// quran.com mirror sizes recorded ${stamp} — paste into QURANCOM_ASSETS sizeBytes`);
    log("{");
    for (const entry of recorded) log(`  ${JSON.stringify(entry.id)}: ${entry.sizeBytes},`);
    log("}");
  }
  return failures;
}

async function run(mode: MirrorMode): Promise<void> {
  log(`qurancom mirror: ${mode} (${QURANCOM_ASSETS.length} assets)`);
  if (mode === "record") {
    await finish(mode, await runRecord());
    return;
  }
  if (mode === "dry-run") {
    await finish(mode, await runDryRun());
    return;
  }
  await finish(mode, await runVerify());
}

async function finish(mode: MirrorMode, failures: readonly string[]): Promise<void> {
  log("");
  if (failures.length > 0) {
    log(`FAILED (${failures.length}):`);
    for (const failure of failures) log(`  - ${failure}`);
    process.exitCode = 1;
    return;
  }
  log(`${mode}: all ${QURANCOM_ASSETS.length} assets ok`);
}

async function main(): Promise<void> {
  await run(parseMode(process.argv.slice(2)));
}

const isDirectRun =
  process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectRun) {
  void main().catch((error) => {
    process.stderr.write(`qurancom mirror failed: ${describeError(error)}\n`);
    process.exitCode = 1;
  });
}
