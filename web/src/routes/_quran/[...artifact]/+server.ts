import { error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import rawTranslations from "$lib/data/translations.json";
import { QURAN_R2_UPSTREAM_BASE } from "$lib/quran/environment";

const TRANSLATION_FILE_PATH = 6;
const IMMUTABLE = "public, max-age=31536000, immutable";
const ALLOWED_ARTIFACTS = new Set<string>([
  "tanzil/arabic/quran-uthmani.sqlite",
  "tanzil/arabic/quran-simple-clean.sqlite",
  ...(rawTranslations as readonly unknown[]).map((row) => {
    if (!Array.isArray(row) || typeof row[TRANSLATION_FILE_PATH] !== "string") {
      throw new Error("[quran-artifact] malformed baked translation catalogue");
    }
    return `tanzil/translations/${row[TRANSLATION_FILE_PATH]}`;
  }),
]);

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
