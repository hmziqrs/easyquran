import { access, constants, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";

import type { RequestHandler } from "./$types";
import { readinessResponse } from "./readiness";

function cacheDir(): string {
  return process.env.QURAN_SSR_CACHE_DIR ?? path.resolve(process.cwd(), ".cache/quran-ssr");
}

async function buildManifestPresent(): Promise<boolean> {
  try {
    await access(path.resolve(process.cwd(), "build/client/_app/version.json"), constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

async function diskCacheWritable(): Promise<boolean> {
  const probe = path.join(cacheDir(), `.health-${process.pid}`);
  try {
    await mkdir(cacheDir(), { recursive: true });
    await writeFile(probe, "");
    await rm(probe, { force: true });
    return true;
  } catch {
    return false;
  }
}

export const GET: RequestHandler = async () => {
  const [manifest, writable] = await Promise.all([buildManifestPresent(), diskCacheWritable()]);
  return readinessResponse(manifest, writable);
};
