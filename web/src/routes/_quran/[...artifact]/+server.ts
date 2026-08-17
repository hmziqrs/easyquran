import rawTranslations from "$lib/data/translations.json";
import { QURAN_R2_UPSTREAM_BASE } from "$lib/quran/environment";
import { error } from "@sveltejs/kit";
import { dev } from "$app/environment";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";

import type { RequestHandler } from "./$types";

const TRANSLATION_FILE_PATH = 6;
const IMMUTABLE = "public, max-age=31536000, immutable";
const ALLOWED_ARTIFACTS = new Set<string>([
  "tanzil/arabic/quran-uthmani.sqlite",
  "tanzil/arabic/quran-simple-clean.sqlite",
  ...rawTranslations.map((row) => {
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- this IS the boundary parse of the baked catalogue row; a non-string artifact path must throw here.
    if (!Array.isArray(row) || typeof row[TRANSLATION_FILE_PATH] !== "string") {
      throw new Error("[quran-artifact] malformed baked translation catalogue");
    }
    return `tanzil/translations/${row[TRANSLATION_FILE_PATH]}`;
  }),
]);

// Dev-only: serve artifacts straight from the locally provisioned db/quran tree so the
// dev server needs neither the Rust API nor R2 reachability. The artifact key is
// allowlist-checked above, so the mapped path cannot escape db/quran. The db/ root is
// probed from cwd upward (web/ and the repo root both work); a miss falls back to the
// R2 proxy.
async function localArtifact(
  artifact: string,
  ifNoneMatch: string | null,
  head: boolean,
): Promise<Response | null> {
  if (!dev) return null;
  const rel = join("db", "quran", artifact.slice("tanzil/".length));
  let bytes: Uint8Array<ArrayBuffer> | null = null;
  let dir = process.cwd();
  for (let depth = 0; depth < 3 && bytes === null; depth++) {
    try {
      const buf = await readFile(resolve(dir, rel));
      bytes = new Uint8Array(buf.byteLength);
      bytes.set(buf);
    } catch {
      dir = dirname(dir);
    }
  }
  if (bytes === null) return null;
  const etag = `W/"quran-artifact:${artifact}"`;
  if (
    ifNoneMatch
      ?.split(",")
      .map((candidate) => candidate.trim())
      .some((candidate) => candidate === "*" || candidate === etag)
  ) {
    return new Response(null, { status: 304, headers: { "cache-control": IMMUTABLE, etag } });
  }
  const headers = new Headers({
    "cache-control": IMMUTABLE,
    "content-type": "application/vnd.sqlite3",
    etag,
  });
  if (head) return new Response(null, { status: 200, headers });
  return new Response(bytes, { status: 200, headers });
}

async function proxyArtifact(
  artifact: string | undefined,
  range: string | null,
  ifNoneMatch: string | null,
  fetcher: typeof fetch,
  head: boolean,
): Promise<Response> {
  if (!artifact || !ALLOWED_ARTIFACTS.has(artifact)) {
    throw error(404, "Unknown Quran artifact");
  }
  const etag = `W/"quran-artifact:${artifact}"`;
  if (
    !range &&
    ifNoneMatch
      ?.split(",")
      .map((candidate) => candidate.trim())
      .some((candidate) => candidate === "*" || candidate === etag)
  ) {
    return new Response(null, {
      status: 304,
      headers: { "cache-control": IMMUTABLE, etag },
    });
  }
  const local = await localArtifact(artifact, range ? null : ifNoneMatch, head);
  if (local) return local;
  const headers = new Headers({ accept: "application/vnd.sqlite3", "accept-encoding": "identity" });
  if (range) headers.set("range", range);
  const upstream = await fetcher(`${QURAN_R2_UPSTREAM_BASE}/${artifact}`, {
    method: head ? "HEAD" : "GET",
    headers,
  });
  if (!upstream.ok && upstream.status !== 206) {
    throw error(502, `Quran artifact upstream returned ${upstream.status}`);
  }
  const responseHeaders = new Headers({
    "cache-control": IMMUTABLE,
    "content-type": upstream.headers.get("content-type") ?? "application/vnd.sqlite3",
    etag,
  });
  for (const name of ["content-length", "content-range", "accept-ranges"]) {
    const value = upstream.headers.get(name);
    if (value) responseHeaders.set(name, value);
  }
  return new Response(head ? null : upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  });
}

export const GET: RequestHandler = ({ params, request, fetch }) =>
  proxyArtifact(
    params.artifact,
    request.headers.get("range"),
    request.headers.get("if-none-match"),
    fetch,
    false,
  );

export const HEAD: RequestHandler = ({ params, request, fetch }) =>
  proxyArtifact(
    params.artifact,
    request.headers.get("range"),
    request.headers.get("if-none-match"),
    fetch,
    true,
  );
