/**
 * upload-sqlite.ts — push the built SQLite files to Cloudflare R2 (S3-compatible).
 *
 *   tsx scripts/upload-sqlite.ts --dry-run   # show what would be uploaded (no creds needed)
 *   tsx scripts/upload-sqlite.ts             # upload, skipping files already present
 *   tsx scripts/upload-sqlite.ts --force     # re-upload everything
 *
 * Keys mirror the repo's tanzil source tree:
 *
 *     tanzil/translations/sqlite/<id>.sqlite
 *     tanzil/arabic/<file>.sqlite
 *
 * Required env:
 *   R2_ACCOUNT_ID
 *   R2_ACCESS_KEY_ID
 *   R2_SECRET_ACCESS_KEY
 */

import { PutObjectCommand, S3Client, ListObjectsV2Command } from "@aws-sdk/client-s3";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

import { ROOT, log, pPool } from "./lib";

const BUCKET = "easyquran";
const PREFIX = "tanzil/";
const SQLITE_DIR = path.join(ROOT, "sqlite"); // .../tanzil/translations/sqlite
const ARABIC_DIR = path.resolve(ROOT, "..", "arabic"); // .../tanzil/arabic

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const force = args.includes("--force");

function requireEnv(key: string): string {
  const v = process.env[key];
  if (!v) throw new Error(`missing required env var: ${key}`);
  return v;
}

/** Local sqlite files → bucket keys that mirror the tanzil tree. */
async function collectItems(): Promise<{ abs: string; key: string }[]> {
  const items: { abs: string; key: string }[] = [];
  for (const [dir, rel] of [
    [SQLITE_DIR, "translations/sqlite"],
    [ARABIC_DIR, "arabic"],
  ] as const) {
    let names: string[] = [];
    try {
      names = (await readdir(dir)).filter((f) => f.endsWith(".sqlite")).sort();
    } catch {
      // dir missing → nothing to upload from it
    }
    for (const f of names) items.push({ abs: path.join(dir, f), key: `${PREFIX}${rel}/${f}` });
  }
  return items;
}

async function main(): Promise<number> {
  const items = await collectItems();
  log(`→ ${items.length} sqlite file(s)`);
  log(`  bucket: ${BUCKET}   prefix: ${PREFIX}   dry-run: ${dryRun}   force: ${force}`);

  if (items.length === 0) {
    log("nothing to upload");
    return 0;
  }

  if (dryRun) {
    for (const it of items) log(`  would upload  ${it.key}`);
    log("\n(dry-run — set R2_* env vars and re-run without --dry-run to upload)");
    return 0;
  }

  const accountId = requireEnv("R2_ACCOUNT_ID");
  const client = new S3Client({
    region: "auto",
    endpoint: `https://${accountId}.r2.cloudflarestorage.com`,
    credentials: {
      accessKeyId: requireEnv("R2_ACCESS_KEY_ID"),
      secretAccessKey: requireEnv("R2_SECRET_ACCESS_KEY"),
    },
    // @aws-sdk/client-s3 v3.729+ adds a CRC32 checksum to every PutObject by
    // default, which Cloudflare R2 rejects. Send checksums only when required.
    requestChecksumCalculation: "WHEN_REQUIRED",
    responseChecksumValidation: "WHEN_REQUIRED",
  });

  log("→ listing existing objects (to skip already-uploaded) ...");
  const existing = new Set<string>();
  let token: string | undefined;
  do {
    const res = await client.send(
      new ListObjectsV2Command({ Bucket: BUCKET, Prefix: PREFIX, ContinuationToken: token }),
    );
    for (const o of res.Contents ?? []) if (o.Key) existing.add(o.Key);
    token = res.IsTruncated ? res.NextContinuationToken : undefined;
  } while (token);

  let uploaded = 0;
  let skipped = 0;
  const failed: { key: string; error: string }[] = [];

  await pPool(
    items,
    async (it) => {
      if (existing.has(it.key) && !force) {
        skipped++;
        return;
      }
      try {
        const body = await readFile(it.abs);
        await client.send(
          new PutObjectCommand({
            Bucket: BUCKET,
            Key: it.key,
            Body: body,
            ContentType: "application/vnd.sqlite3",
            CacheControl: "public, max-age=31536000, immutable",
          }),
        );
        uploaded++;
        log(`  ✓ ${it.key}  (${(body.length / 1024).toFixed(0)} KB)`);
      } catch (e) {
        failed.push({ key: it.key, error: e instanceof Error ? e.message : String(e) });
      }
    },
    6,
  );

  log(`\nuploaded: ${uploaded}   skipped: ${skipped}   failed: ${failed.length}   total: ${items.length}`);
  if (failed.length) {
    log("failures:");
    for (const f of failed) log(`  ✗ ${f.key}: ${f.error}`);
  }
  return failed.length ? 1 : 0;
}

main().catch((e) => {
  console.error(`✗ ${e instanceof Error ? e.message : e}`);
  process.exit(1);
});
