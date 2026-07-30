/**
 * upload.ts — push the mirrored translations to Cloudflare R2 (S3-compatible).
 *
 *   tsx scripts/upload.ts --dry-run      # show what would be uploaded, no creds needed
 *   tsx scripts/upload.ts                # upload sql/*.sql + index.min.json (skips existing)
 *   tsx scripts/upload.ts --force        # re-upload even if the key already exists
 *   tsx scripts/upload.ts --full         # also upload the fat index.json
 *
 * Configuration (env):
 *   R2_ACCOUNT_ID         required (unless R2_ENDPOINT is set)
 *   R2_ACCESS_KEY_ID      required
 *   R2_SECRET_ACCESS_KEY  required
 *   R2_BUCKET             default: easyquran
 *   R2_KEY_PREFIX         default: translations/   (keys become <prefix>sql/<id>.sql …)
 *   R2_ENDPOINT           default: https://<account>.r2.cloudflarestorage.com
 *   R2_PUBLIC_BASE        optional, e.g. https://cdn.easyquran.app — used to print URLs
 */

import { PutObjectCommand, S3Client, ListObjectsV2Command } from "@aws-sdk/client-s3";
import { readFile } from "node:fs/promises";
import path from "node:path";

import { INDEX, INDEX_MIN, ROOT, SQLDIR, listSqlFiles, log } from "./lib";

interface Args {
  dryRun: boolean;
  force: boolean;
  full: boolean;
}

function parseArgs(argv: string[]): Args {
  return {
    dryRun: argv.includes("--dry-run"),
    force: argv.includes("--force"),
    full: argv.includes("--full"),
  };
}

function requireEnv(key: string): string {
  const v = process.env[key];
  if (!v) throw new Error(`missing required env var: ${key}`);
  return v;
}

interface UploadItem {
  abs: string;
  key: string;
  contentType: string;
}

function collectItems(prefix: string, full: boolean): UploadItem[] {
  const items: UploadItem[] = listSqlFiles().map((f) => ({
    abs: path.join(SQLDIR, f),
    key: `${prefix}sql/${f}`,
    contentType: "text/plain; charset=utf-8",
  }));
  items.push({
    abs: INDEX_MIN,
    key: `${prefix}index.min.json`,
    contentType: "application/json; charset=utf-8",
  });
  if (full) {
    items.push({
      abs: INDEX,
      key: `${prefix}index.json`,
      contentType: "application/json; charset=utf-8",
    });
  }
  return items;
}

async function listExisting(client: S3Client, bucket: string, prefix: string): Promise<Set<string>> {
  const keys = new Set<string>();
  let token: string | undefined;
  do {
    const res = await client.send(
      new ListObjectsV2Command({ Bucket: bucket, Prefix: prefix, ContinuationToken: token }),
    );
    for (const o of res.Contents ?? []) if (o.Key) keys.add(o.Key);
    token = res.IsTruncated ? res.NextContinuationToken : undefined;
  } while (token);
  return keys;
}

async function main(): Promise<number> {
  const { dryRun, force, full } = parseArgs(process.argv.slice(2));
  const bucket = process.env.R2_BUCKET ?? "easyquran";
  const prefix = process.env.R2_KEY_PREFIX ?? "translations/";

  const items = collectItems(prefix, full);
  log(`→ ${items.length} local file(s) under ${ROOT}`);
  log(`  bucket: ${bucket}   prefix: ${prefix}   dry-run: ${dryRun}   force: ${force}`);

  if (dryRun) {
    for (const it of items) log(`  would upload  ${it.key}  (${it.contentType})`);
    log("\n(dry-run — set R2_* env vars and re-run without --dry-run to upload)");
    return 0;
  }

  // real run — need credentials + a client
  const accountId = process.env.R2_ACCOUNT_ID;
  const endpoint = process.env.R2_ENDPOINT ?? (accountId ? `https://${accountId}.r2.cloudflarestorage.com` : undefined);
  if (!endpoint) throw new Error("set R2_ACCOUNT_ID (or R2_ENDPOINT)");
  const accessKeyId = requireEnv("R2_ACCESS_KEY_ID");
  const secretAccessKey = requireEnv("R2_SECRET_ACCESS_KEY");

  const client = new S3Client({
    region: "auto",
    endpoint,
    credentials: { accessKeyId, secretAccessKey },
    // @aws-sdk/client-s3 v3.729+ adds a CRC32 checksum to every PutObject by
    // default, which Cloudflare R2 rejects (BadDigest / signature mismatch).
    // Send checksums only when the operation requires it — safe for R2 and S3.
    requestChecksumCalculation: "WHEN_REQUIRED",
    responseChecksumValidation: "WHEN_REQUIRED",
  });

  log("→ listing existing objects (to skip already-uploaded) ...");
  const existing = await listExisting(client, bucket, prefix);

  let uploaded = 0;
  let skipped = 0;
  for (const it of items) {
    if (existing.has(it.key) && !force) {
      skipped++;
      continue;
    }
    const body = await readFile(it.abs);
    await client.send(
      new PutObjectCommand({
        Bucket: bucket,
        Key: it.key,
        Body: body,
        ContentType: it.contentType,
        CacheControl: "public, max-age=31536000, immutable",
      }),
    );
    uploaded++;
    log(`  ✓ ${it.key}  (${(body.length / 1024).toFixed(0)} KB)`);
  }

  log(`\nuploaded: ${uploaded}   skipped: ${skipped}   total: ${items.length}`);
  const base = process.env.R2_PUBLIC_BASE;
  if (base) {
    const cleanBase = base.replace(/\/$/, "");
    log(`catalog url: ${cleanBase}/${prefix}index.min.json`);
  }
  return 0;
}

main().catch((e) => {
  console.error(`✗ ${e instanceof Error ? e.message : e}`);
  process.exit(1);
});
