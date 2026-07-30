/**
 * upload-sqlite.ts — push the built SQLite files to Cloudflare R2 (S3-compatible).
 *
 *   tsx scripts/upload-sqlite.ts --dry-run      # show what would be uploaded, no creds needed
 *   tsx scripts/upload-sqlite.ts                # upload sqlite/*.sqlite + arabic/*.sqlite (skips existing)
 *   tsx scripts/upload-sqlite.ts --force        # re-upload even if the key already exists
 *   tsx scripts/upload-sqlite.ts translations   # only the translation sqlite files
 *   tsx scripts/upload-sqlite.ts arabic         # only the arabic sqlite files
 *
 * Keys mirror the repo's tanzil source tree so the bucket reads like the sources:
 *
 *     tanzil/translations/sqlite/<id>.sqlite
 *     tanzil/arabic/<file>.sqlite
 *
 * (`tanzil/` is the `db/quran/tanzil/` dir; override the root with R2_KEY_PREFIX.)
 *
 * Configuration (env):
 *   R2_ACCOUNT_ID         required (unless R2_ENDPOINT is set)
 *   R2_ACCESS_KEY_ID      required
 *   R2_SECRET_ACCESS_KEY  required
 *   R2_BUCKET             default: easyquran
 *   R2_KEY_PREFIX         default: tanzil/   (keys become <prefix>translations/sqlite/<id>.sqlite …)
 *   R2_ENDPOINT           default: https://<account>.r2.cloudflarestorage.com
 *   R2_PUBLIC_BASE        optional, e.g. https://cdn.easyquran.app — used to print URLs
 */

import { PutObjectCommand, S3Client, ListObjectsV2Command } from "@aws-sdk/client-s3";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

import { ROOT, log, pPool } from "./lib";

/** .../db/quran/tanzil */
const TANZIL_ROOT = path.resolve(ROOT, "..");
const SQLITE_DIR = path.join(ROOT, "sqlite");
const ARABIC_DIR = path.join(TANZIL_ROOT, "arabic");

const SQLITE_CONTENT_TYPE = "application/vnd.sqlite3";

interface Args {
  dryRun: boolean;
  force: boolean;
  scope: "all" | "translations" | "arabic";
}

function parseArgs(argv: string[]): Args {
  const positional = argv.filter((a) => !a.startsWith("--"));
  const scope = (positional[0] as Args["scope"] | undefined) ?? "all";
  if (scope !== "all" && scope !== "translations" && scope !== "arabic") {
    throw new Error(`unknown scope "${scope}"; use: translations | arabic`);
  }
  return {
    dryRun: argv.includes("--dry-run"),
    force: argv.includes("--force"),
    scope,
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
}

/** Collect the local sqlite files into bucket keys that mirror the tanzil tree. */
async function collectItems(prefix: string, scope: Args["scope"]): Promise<UploadItem[]> {
  const items: UploadItem[] = [];

  const addDir = async (dir: string, rel: string): Promise<void> => {
    let names: string[];
    try {
      names = (await readdir(dir)).filter((f) => f.endsWith(".sqlite")).sort();
    } catch {
      names = [];
    }
    for (const f of names) {
      items.push({ abs: path.join(dir, f), key: `${prefix}${rel}/${f}` });
    }
  };

  if (scope === "all" || scope === "translations") await addDir(SQLITE_DIR, "translations/sqlite");
  if (scope === "all" || scope === "arabic") await addDir(ARABIC_DIR, "arabic");
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

interface UploadResult {
  uploaded: number;
  skipped: number;
  failed: { key: string; error: string }[];
}

async function main(): Promise<number> {
  const args = parseArgs(process.argv.slice(2));
  const bucket = process.env.R2_BUCKET ?? "easyquran";
  const prefix = process.env.R2_KEY_PREFIX ?? "tanzil/";

  const items = await collectItems(prefix, args.scope);
  log(`→ ${items.length} sqlite file(s) [scope: ${args.scope}]`);
  log(`  bucket: ${bucket}   prefix: ${prefix}   dry-run: ${args.dryRun}   force: ${args.force}`);

  if (items.length === 0) {
    log("nothing to upload");
    return 0;
  }

  if (args.dryRun) {
    for (const it of items) log(`  would upload  ${it.key}`);
    log("\n(dry-run — set R2_* env vars and re-run without --dry-run to upload)");
    return 0;
  }

  // real run — need credentials + a client
  const accountId = process.env.R2_ACCOUNT_ID;
  const endpoint =
    process.env.R2_ENDPOINT ?? (accountId ? `https://${accountId}.r2.cloudflarestorage.com` : undefined);
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
  const failed: { key: string; error: string }[] = [];

  await pPool(
    items,
    async (it): Promise<void> => {
      if (existing.has(it.key) && !args.force) {
        skipped++;
        return;
      }
      try {
        const body = await readFile(it.abs);
        await client.send(
          new PutObjectCommand({
            Bucket: bucket,
            Key: it.key,
            Body: body,
            ContentType: SQLITE_CONTENT_TYPE,
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
  const base = process.env.R2_PUBLIC_BASE;
  if (base && (args.scope === "all" || args.scope === "translations")) {
    log(`example url: ${base.replace(/\/$/, "")}/${prefix}translations/sqlite/en.sahih.sqlite`);
  }
  return failed.length ? 1 : 0;
}

main().catch((e) => {
  console.error(`✗ ${e instanceof Error ? e.message : e}`);
  process.exit(1);
});
