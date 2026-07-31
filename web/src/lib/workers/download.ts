/* ════════════════════════════════════════════════════════════════════════
   download.ts — GENERIC, worker-safe fetch + verify primitives.

   No $lib/$env/$app, no Svelte, no DOM-only APIs, no Quran types. Only
   relative imports + web-standard APIs (fetch, crypto.subtle, streams).
   ════════════════════════════════════════════════════════════════════════ */

/** What to download and (optionally) how to check it. */
export interface DownloadSpec {
  url: string;
  /** Expected byte length; checked post-download if set. */
  sizeBytes?: number;
  /** Expected SHA-256 hex digest; checked post-download if set. */
  sha256?: string;
  /** Human label used in error messages (falls back to url). */
  label?: string;
}

/** Bytes-received progress. `total` is 0 when unknown. */
export interface Progress {
  loaded: number;
  total: number;
}

export type ProgressFn = (p: Progress) => void;

/** Compute the SHA-256 hex digest of a byte buffer using WebCrypto. */
export async function sha256Hex(data: BufferSource): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  let out = "";
  for (const b of new Uint8Array(digest)) out += b.toString(16).padStart(2, "0");
  return out;
}

/**
 * Verify a buffer against a DownloadSpec's optional size + sha256. Throws (with
 * spec.label or url in the message) on mismatch; returns the buffer unchanged.
 * Only the constraints that are present are checked.
 */
export async function verifyBytes<B extends Uint8Array<ArrayBuffer>>(
  buf: B,
  spec: DownloadSpec,
): Promise<B> {
  const name = spec.label ?? spec.url;
  if (spec.sizeBytes !== undefined && buf.byteLength !== spec.sizeBytes) {
    throw new Error(`${name}: size ${buf.byteLength} ≠ expected ${spec.sizeBytes}`);
  }
  if (spec.sha256 !== undefined && (await sha256Hex(buf)) !== spec.sha256) {
    throw new Error(`${name}: sha256 mismatch`);
  }
  return buf;
}

/**
 * Fetch a URL with identity encoding, stream the body for progress, assemble it
 * into one buffer, and verify size/sha256 when either is present.
 *
 * The expected total is `spec.sizeBytes` (0 if unknown) — `Content-Length` is
 * never read, so this introduces no cross-origin ExposeHeaders/CORS need.
 * `onProgress` is invoked once with `{loaded:0,total}` up front and then per
 * chunk as bytes arrive. Non-2xx → throw. The reader is released in a finally
 * whether we finished, threw, or aborted. Verification runs ONLY if
 * `spec.sizeBytes` or `spec.sha256` is set.
 */
export async function downloadBytes(
  spec: DownloadSpec,
  onProgress?: ProgressFn,
): Promise<Uint8Array<ArrayBuffer>> {
  const res = await fetch(spec.url, { headers: { "Accept-Encoding": "identity" } });
  if (!res.ok) throw new Error(`fetch ${spec.label ?? spec.url}: HTTP ${res.status}`);

  const total = spec.sizeBytes ?? 0;
  let bytes: Uint8Array<ArrayBuffer>;

  if (res.body) {
    const reader = res.body.getReader();
    try {
      const chunks: Uint8Array[] = [];
      let loaded = 0;
      onProgress?.({ loaded: 0, total });
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          chunks.push(value);
          loaded += value.byteLength;
          onProgress?.({ loaded, total });
        }
      }
      // Concatenate chunks into one buffer (same shape as arrayBuffer()).
      bytes = new Uint8Array(loaded);
      let off = 0;
      for (const c of chunks) {
        bytes.set(c, off);
        off += c.byteLength;
      }
    } finally {
      // Release the body stream whether we finished, threw, or aborted.
      await reader.cancel().catch(() => {});
    }
  } else {
    // No body stream (e.g. opaque/unsupported response): buffer at once, no progress.
    bytes = new Uint8Array(await res.arrayBuffer());
  }

  if (spec.sizeBytes !== undefined || spec.sha256 !== undefined) {
    await verifyBytes(bytes, spec);
  }
  return bytes;
}
