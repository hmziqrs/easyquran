export interface DownloadSpec {
  url: string;
  sizeBytes: number;
  label?: string;
}

export interface Progress {
  loaded: number;
  total: number;
}

export type ProgressFn = (p: Progress) => void;

export const DOWNLOAD_BUDGET_MS = 120_000;

export function unsafeDownloadSpec(spec: {
  url: string;
  sizeBytes?: number;
  label?: string;
}): DownloadSpec {
  // SAFETY: trusted-caller escape hatch by design (see fn name); every DownloadSpec
  // consumer re-guards `spec.sizeBytes !== undefined`, so an absent sizeBytes only
  // disables size enforcement.
  return spec as DownloadSpec;
}

export async function verifyBytes<B extends Uint8Array<ArrayBuffer>>(
  buf: B,
  spec: DownloadSpec,
): Promise<B> {
  const name = spec.label ?? spec.url;
  if (spec.sizeBytes !== undefined && buf.byteLength !== spec.sizeBytes) {
    throw new Error(`${name}: size ${buf.byteLength} ≠ expected ${spec.sizeBytes}`);
  }
  return buf;
}

export async function downloadBytes(
  spec: DownloadSpec,
  onProgress?: ProgressFn,
): Promise<Uint8Array<ArrayBuffer>> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), DOWNLOAD_BUDGET_MS);
  try {
    const res = await fetch(spec.url, {
      headers: { "Accept-Encoding": "identity" },
      signal: controller.signal,
    });
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
            if (
              spec.sizeBytes !== undefined &&
              spec.sizeBytes > 0 &&
              loaded + value.byteLength > spec.sizeBytes
            ) {
              controller.abort();
              throw new Error(
                `${spec.label ?? spec.url}: stream exceeded declared ${spec.sizeBytes} bytes`,
              );
            }
            chunks.push(value);
            loaded += value.byteLength;
            onProgress?.({ loaded, total });
          }
        }
        bytes = new Uint8Array(loaded);
        let off = 0;
        for (const c of chunks) {
          bytes.set(c, off);
          off += c.byteLength;
        }
      } finally {
        await reader.cancel().catch(() => {});
      }
    } else {
      // No streaming body (some test fixtures, or an env without ReadableStream
      // bodies). res.arrayBuffer() materializes the FULL response in one
      // allocation, so enforce the ceiling before that allocation when the server
      // reports Content-Length honestly, and cap the result afterward — both with
      // the same `> spec.sizeBytes` semantics as the streaming path, and before
      // verifyBytes would otherwise accept the buffer.
      const declared = res.headers ? res.headers.get("content-length") : null;
      if (declared !== null) {
        const cl = Number.parseInt(declared, 10);
        if (
          spec.sizeBytes !== undefined &&
          spec.sizeBytes > 0 &&
          Number.isFinite(cl) &&
          cl > spec.sizeBytes
        ) {
          controller.abort();
          throw new Error(
            `${spec.label ?? spec.url}: Content-Length ${cl} exceeds declared ${spec.sizeBytes} bytes`,
          );
        }
      }
      const buf = await res.arrayBuffer();
      if (spec.sizeBytes !== undefined && spec.sizeBytes > 0 && buf.byteLength > spec.sizeBytes) {
        controller.abort();
        throw new Error(
          `${spec.label ?? spec.url}: body ${buf.byteLength} exceeds declared ${spec.sizeBytes} bytes`,
        );
      }
      bytes = new Uint8Array(buf);
    }

    await verifyBytes(bytes, spec);
    return bytes;
  } finally {
    clearTimeout(timer);
  }
}
