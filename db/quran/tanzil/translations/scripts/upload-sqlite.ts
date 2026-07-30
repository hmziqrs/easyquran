/**
 * upload-sqlite.ts — push the built SQLite files and the web catalog to
 * Cloudflare R2 (S3-compatible). This is the only publisher for this dataset.
 *
 *   tsx scripts/upload-sqlite.ts --dry-run   # show what would be uploaded (no creds needed)
 *   tsx scripts/upload-sqlite.ts             # upload, skipping files already present
 *   tsx scripts/upload-sqlite.ts --force     # re-upload everything
 *
 * Keys mirror the repo's tanzil source tree:
 *
 *     tanzil/translations/index.min.json     catalog — always re-uploaded
 *     tanzil/translations/sqlite/<id>.sqlite
 *     tanzil/arabic/<file>.sqlite
 *
 * The raw MySQL dumps under sql/ are deliberately NOT published: they are the
 * upstream mirror format, already versioned in git, and nothing downstream can
 * read them. Only the SQLite artifacts ship, and the catalog's `file` field
 * points at them relative to its own key.
 *
 * Required env:
 *   R2_ACCOUNT_ID          (or R2_ENDPOINT)
 *   R2_ACCESS_KEY_ID
 *   R2_SECRET_ACCESS_KEY
 *
 * Optional env:
 *   R2_BUCKET              default: easyquran
 *   R2_ENDPOINT            default: https://<account>.r2.cloudflarestorage.com
 *   R2_PUBLIC_BASE         e.g. https://cdn.easyquran.app — prints the catalog URL
 */

import { PutObjectCommand, S3Client, ListObjectsV2Command } from "@aws-sdk/client-s3";
import { existsSync } from "node:fs";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

import { INDEX_MIN, ROOT, log, pPool } from "./lib";

const BUCKET = process.env.R2_BUCKET ?? "easyquran";
const PREFIX = "tanzil/";
const SQLITE_DIR = path.join(ROOT, "sqlite"); // .../tanzil/translations/sqlite
const ARABIC_DIR = path.resolve(ROOT, "..", "arabic"); // .../tanzil/arabic

/** one immutable object per translation id — safe to cache forever */
const IMMUTABLE = "public, max-age=31536000, immutable";
/** the catalog is overwritten whenever the translation set changes */
const CATALOG_CACHE = "public, max-age=300, must-revalidate";

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const force = args.includes("--force");

interface Item {
  abs: string;
  key: string;
  contentType: string;
  cacheControl: string;
  /** re-uploaded every run — the exists-check never skips it */
  mutable?: boolean;
}

function requireEnv(key: string): string {
  const v = process.env[key];
  if (!v) throw new Error(`missing required env var: ${key}`);
  return v;
}

/** Local sqlite files + the catalog → bucket keys that mirror the tanzil tree. */
async function collectItems(): Promise<Item[]> {
  const items: Item[] = [];

  if (existsSync(INDEX_MIN)) {
    items.push({
      abs: INDEX_MIN,
      key: `${PREFIX}translations/index.min.json`,
      contentType: "application/json; charset=utf-8",
      cacheControl: CATALOG_CACHE,
      mutable: true,
    });
  } else {
    log("  ! index.min.json missing — run `npm run catalog` first (catalog not uploaded)");
  }

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
    if (names.length === 0) log(`  ! no .sqlite files under ${path.relative(ROOT, dir)}`);
    for (const f of names) {
      items.push({
        abs: path.join(dir, f),
        key: `${PREFIX}${rel}/${f}`,
        contentType: "application/vnd.sqlite3",
        cacheControl: IMMUTABLE,
      });
    }
  }

  return items;
}

async function main(): Promise<number> {
  const items = await collectItems();
  log(`→ ${items.length} object(s)`);
  log(`  bucket: ${BUCKET}   prefix: ${PREFIX}   dry-run: ${dryRun}   force: ${force}`);

  if (items.length === 0) {
    log("nothing to upload");
    return 0;
  }

  if (dryRun) {
    for (const it of items) log(`  would upload  ${it.key}${it.mutable ? "  (always)" : ""}`);
    log("\n(dry-run — set R2_* env vars and re-run without --dry-run to upload)");
    return 0;
  }

  const endpoint =
    process.env.R2_ENDPOINT ??
    (process.env.R2_ACCOUNT_ID ? `https://${process.env.R2_ACCOUNT_ID}.r2.cloudflarestorage.com` : undefined);
  if (!endpoint) throw new Error("set R2_ACCOUNT_ID (or R2_ENDPOINT)");

  const client = new S3Client({
    region: "auto",
    endpoint,
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
      if (existing.has(it.key) && !force && !it.mutable) {
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
            ContentType: it.contentType,
            CacheControl: it.cacheControl,
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

  const base = process.env.R2_PUBLIC_BASE;
  if (base) log(`catalog url: ${base.replace(/\/$/, "")}/${PREFIX}translations/index.min.json`);

  return failed.length ? 1 : 0;
}

main().catch((e) => {
  console.error(`✗ ${e instanceof Error ? e.message : e}`);
  process.exit(1);
});
