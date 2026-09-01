/**
 * Single publisher for every production Quran artifact.
 *
 * Local Quran data stays under ignored db/. Tracked baked maps own artifact identity:
 * id/path + byte size, never a digest. This script reads files without changing them.
 */

import type { FileHandle } from "node:fs/promises";
import { open } from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";
import { fileURLToPath } from "node:url";

import type { PutObjectCommandInput } from "@aws-sdk/client-s3";

import { TRANSLATIONS } from "../../web/src/lib/data/translations.ts";
import { registeredSourceProfiles } from "../../web/src/lib/quran/view/source-profiles.ts";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const QURAN_ROOT = path.join(REPO_ROOT, "db", "quran");
const METADATA_PATH = path.join(QURAN_ROOT, "quran-data.xml");
const BUCKET = process.env.R2_BUCKET ?? "easyquran";
const PREFIX = "tanzil/";
const TRANSLATION_PREFIX = `${PREFIX}translations/`;
const IMMUTABLE_CACHE = "public, max-age=31536000, immutable";
const CATALOG_CACHE = "public, max-age=300, must-revalidate";
const CONCURRENCY = 6;
const QURAN_DATA_SIZE_BYTES = 77_234;
const SQLITE_HEADER = "SQLite format 3\0";

export interface UploadItem {
  readonly absolutePath: string | null;
  readonly generatedBody: Buffer | null;
  readonly key: string;
  readonly sizeBytes: number;
  readonly contentType: string;
  readonly cacheControl: string;
  readonly mutable: boolean;
  readonly sqlite: boolean;
}

interface UploadResult {
  readonly uploaded: number;
  readonly skipped: number;
  readonly failures: readonly string[];
}

function log(message: string): void {
  process.stdout.write(`${message}\n`);
}

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`missing required env var: ${name}`);
  return value;
}

function parseArgs(): { dryRun: boolean } {
  const args = process.argv.slice(2);
  const supported = new Set(["--", "--dry-run"]);
  const unknown = args.filter((arg) => !supported.has(arg));
  if (unknown.length > 0) throw new Error(`unknown option(s): ${unknown.join(", ")}`);
  return { dryRun: args.includes("--dry-run") };
}

function translationLocalPath(artifactPath: string): string {
  if (!artifactPath.startsWith(TRANSLATION_PREFIX)) {
    throw new Error(`translation artifact outside ${TRANSLATION_PREFIX}: ${artifactPath}`);
  }
  const relativePath = artifactPath.slice(TRANSLATION_PREFIX.length);
  const isTanzil = relativePath.startsWith("sqlite/");
  const isQuranEnc = relativePath.startsWith("quranenc/sqlite/");
  if (!isTanzil && !isQuranEnc) {
    throw new Error(`unsupported translation artifact path: ${artifactPath}`);
  }
  if (path.posix.normalize(relativePath) !== relativePath) {
    throw new Error(`non-canonical translation artifact path: ${artifactPath}`);
  }
  return path.join(QURAN_ROOT, "translations", relativePath);
}

export function buildCatalogBody(): Buffer {
  const catalogue = TRANSLATIONS.map((translation) => ({
    id: translation.id,
    language: translation.language,
    languageCode: translation.languageCode,
    direction: translation.direction,
    name: translation.name,
    translator: translation.translator,
    file: {
      path: translation.filePath,
      sizeBytes: translation.sizeBytes,
    },
  }));
  return Buffer.from(`${JSON.stringify(catalogue, null, 2)}\n`);
}

async function validateOpenFile(item: UploadItem, handle: FileHandle): Promise<void> {
  const details = await handle.stat();
  if (!details.isFile()) throw new Error(`${item.absolutePath ?? item.key} is not a file`);
  if (details.size !== item.sizeBytes) {
    throw new Error(`${item.key}: local size ${details.size}, expected ${item.sizeBytes}`);
  }
  if (item.sqlite) {
    const buffer = Buffer.alloc(SQLITE_HEADER.length);
    const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
    if (bytesRead !== buffer.length || buffer.toString("latin1") !== SQLITE_HEADER) {
      throw new Error(`${item.key}: invalid SQLite header`);
    }
  }
}

async function validateItem(item: UploadItem): Promise<void> {
  if (item.generatedBody !== null) {
    if (item.generatedBody.length !== item.sizeBytes) {
      throw new Error(`${item.key}: generated size does not match upload plan`);
    }
    return;
  }
  if (item.absolutePath === null) throw new Error(`${item.key}: missing upload source`);
  const handle = await open(item.absolutePath, "r");
  try {
    await validateOpenFile(item, handle);
  } finally {
    await handle.close();
  }
}

async function collectItems(): Promise<readonly UploadItem[]> {
  const catalogBody = buildCatalogBody();
  const items: UploadItem[] = [];

  for (const profile of registeredSourceProfiles()) {
    items.push({
      absolutePath: path.join(REPO_ROOT, profile.artifact.repositoryPath),
      generatedBody: null,
      key: profile.artifact.r2Path,
      sizeBytes: profile.artifact.sizeBytes,
      contentType: "application/vnd.sqlite3",
      cacheControl: IMMUTABLE_CACHE,
      mutable: false,
      sqlite: true,
    });
  }

  for (const translation of TRANSLATIONS) {
    items.push({
      absolutePath: translationLocalPath(translation.artifactPath),
      generatedBody: null,
      key: translation.artifactPath,
      sizeBytes: translation.sizeBytes,
      contentType: "application/vnd.sqlite3",
      cacheControl: IMMUTABLE_CACHE,
      mutable: false,
      sqlite: true,
    });
  }

  items.push(
    {
      absolutePath: METADATA_PATH,
      generatedBody: null,
      key: `${PREFIX}quran-data.xml`,
      sizeBytes: QURAN_DATA_SIZE_BYTES,
      contentType: "application/xml; charset=utf-8",
      cacheControl: IMMUTABLE_CACHE,
      mutable: false,
      sqlite: false,
    },
    {
      absolutePath: null,
      generatedBody: catalogBody,
      key: `${TRANSLATION_PREFIX}index.min.json`,
      sizeBytes: catalogBody.length,
      contentType: "application/json; charset=utf-8",
      cacheControl: CATALOG_CACHE,
      mutable: true,
      sqlite: false,
    },
  );

  const keys = new Set(items.map((item) => item.key));
  if (keys.size !== items.length) throw new Error("duplicate R2 keys in upload plan");
  await Promise.all(items.map(validateItem));
  return items.sort((left, right) => {
    if (left.mutable && !right.mutable) return 1;
    if (!left.mutable && right.mutable) return -1;
    return left.key.localeCompare(right.key);
  });
}

export async function listExisting(
  client: import("@aws-sdk/client-s3").S3Client,
): Promise<Map<string, number>> {
  const { ListObjectsV2Command } = await import("@aws-sdk/client-s3");
  const existing = new Map<string, number>();
  const seenTokens = new Set<string>();
  let token: string | undefined;
  while (true) {
    const response = await client.send(
      new ListObjectsV2Command({ Bucket: BUCKET, Prefix: PREFIX, ContinuationToken: token }),
    );
    for (const object of response.Contents ?? []) {
      if (!object.Key) throw new Error("R2 listing returned an object without a key");
      if (!Number.isSafeInteger(object.Size) || (object.Size ?? -1) < 0) {
        throw new Error(`${object.Key}: R2 listing returned an invalid object size`);
      }
      if (existing.has(object.Key)) throw new Error(`${object.Key}: duplicate key in R2 listing`);
      existing.set(object.Key, object.Size as number);
    }
    if (!response.IsTruncated) break;
    const nextToken = response.NextContinuationToken;
    if (!nextToken) throw new Error("R2 listing was truncated without a continuation token");
    if (seenTokens.has(nextToken)) throw new Error("R2 listing repeated a continuation token");
    seenTokens.add(nextToken);
    token = nextToken;
  }
  return existing;
}

async function runPool<T>(values: readonly T[], worker: (value: T) => Promise<void>): Promise<void> {
  let nextIndex = 0;
  async function runWorker(): Promise<void> {
    while (nextIndex < values.length) {
      const index = nextIndex;
      nextIndex += 1;
      const value = values[index];
      if (value) await worker(value);
    }
  }
  const workerCount = Math.min(CONCURRENCY, values.length);
  await Promise.all(Array.from({ length: workerCount }, runWorker));
}

function assertRemoteSize(
  item: UploadItem,
  exists: boolean,
  remoteSize: number | undefined,
): void {
  if (!exists || item.mutable) return;
  if (remoteSize === undefined) throw new Error(`${item.key}: R2 listing omitted object size`);
  if (remoteSize === item.sizeBytes) return;
  throw new Error(
    `${item.key}: remote size ${remoteSize}, local expected size ${item.sizeBytes}; immutable object cannot be replaced`,
  );
}

export function putObjectInput(item: UploadItem, body: Readable): PutObjectCommandInput {
  const input: PutObjectCommandInput = {
    Bucket: BUCKET,
    Key: item.key,
    Body: body,
    ContentLength: item.sizeBytes,
    ContentType: item.contentType,
    CacheControl: item.cacheControl,
  };
  if (!item.mutable) input.IfNoneMatch = "*";
  return input;
}

export async function uploadItems(
  client: import("@aws-sdk/client-s3").S3Client,
  items: readonly UploadItem[],
): Promise<UploadResult> {
  const { PutObjectCommand } = await import("@aws-sdk/client-s3");
  log("→ listing existing R2 objects ...");
  const existing = await listExisting(client);
  for (const item of items) {
    assertRemoteSize(item, existing.has(item.key), existing.get(item.key));
  }
  let uploaded = 0;
  let skipped = 0;
  const failures: string[] = [];

  async function uploadItem(item: UploadItem): Promise<void> {
    let handle: FileHandle | null = null;
    let body: Readable | null = null;
    try {
      if (existing.has(item.key) && !item.mutable) {
        skipped += 1;
        return;
      }
      if (item.generatedBody !== null) {
        body = Readable.from([item.generatedBody]);
      } else {
        if (item.absolutePath === null) throw new Error(`${item.key}: missing upload source`);
        handle = await open(item.absolutePath, "r");
        await validateOpenFile(item, handle);
        body = handle.createReadStream({ autoClose: false });
      }
      await client.send(new PutObjectCommand(putObjectInput(item, body)));
      uploaded += 1;
      log(`  ✓ ${item.key} (${Math.ceil(item.sizeBytes / 1024)} KiB)`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      failures.push(`${item.key}: ${message}`);
    } finally {
      body?.destroy();
      await handle?.close();
    }
  }

  const immutableItems = items.filter((item) => !item.mutable);
  const mutableItems = items.filter((item) => item.mutable);
  await runPool(immutableItems, uploadItem);
  if (failures.length === 0) {
    await runPool(mutableItems, uploadItem);
  } else {
    for (const item of mutableItems) {
      failures.push(`${item.key}: not uploaded because an immutable artifact failed`);
    }
  }

  return { uploaded, skipped, failures: failures.sort() };
}

async function main(): Promise<void> {
  const { dryRun } = parseArgs();
  const items = await collectItems();
  log(`→ ${items.length} object(s)`);
  log(`  bucket: ${BUCKET}   prefix: ${PREFIX}   dry-run: ${dryRun}`);

  if (dryRun) {
    for (const item of items) {
      const marker = item.mutable ? " (always)" : "";
      log(`  would upload ${item.key}${marker}`);
    }
    return;
  }

  const endpoint = process.env.R2_ENDPOINT ?? resolveAccountEndpoint();
  if (!endpoint) throw new Error("set R2_ACCOUNT_ID or R2_ENDPOINT");
  const { S3Client } = await import("@aws-sdk/client-s3");
  const client = new S3Client({
    region: "auto",
    endpoint,
    credentials: {
      accessKeyId: requireEnv("R2_ACCESS_KEY_ID"),
      secretAccessKey: requireEnv("R2_SECRET_ACCESS_KEY"),
    },
    requestChecksumCalculation: "WHEN_REQUIRED",
    responseChecksumValidation: "WHEN_REQUIRED",
    maxAttempts: 1,
  });
  let result: UploadResult;
  try {
    result = await uploadItems(client, items);
  } finally {
    client.destroy();
  }
  log(
    `uploaded: ${result.uploaded}   skipped: ${result.skipped}   failed: ${result.failures.length}   total: ${items.length}`,
  );
  for (const failure of result.failures) log(`  ✗ ${failure}`);
  if (result.failures.length > 0) process.exitCode = 1;

  const publicBase = process.env.R2_PUBLIC_BASE;
  if (publicBase && result.failures.length === 0) {
    log(`catalog url: ${publicBase.replace(/\/$/, "")}/${TRANSLATION_PREFIX}index.min.json`);
  }
}

function resolveAccountEndpoint(): string | undefined {
  const accountId = process.env.R2_ACCOUNT_ID;
  if (!accountId) return undefined;
  return `https://${accountId}.r2.cloudflarestorage.com`;
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : undefined;
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`✗ ${message}\n`);
    process.exitCode = 1;
  });
}
