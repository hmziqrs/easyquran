export interface DownloadSpec {
  url: string;
  sizeBytes?: number;
  sha256?: string;
  label?: string;
}

export interface Progress {
  loaded: number;
  total: number;
}

export type ProgressFn = (p: Progress) => void;

export async function sha256Hex(data: BufferSource): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  let out = "";
  for (const b of new Uint8Array(digest)) out += b.toString(16).padStart(2, "0");
  return out;
}

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
    bytes = new Uint8Array(await res.arrayBuffer());
  }

  if (spec.sizeBytes !== undefined || spec.sha256 !== undefined) {
    await verifyBytes(bytes, spec);
  }
  return bytes;
}
