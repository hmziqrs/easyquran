import { downloadBytes, verifyBytes } from "./download";
import type { DownloadSpec, ProgressFn } from "./download";
import type { ByteStore } from "./storage";

export interface EnsureOptions {
  store?: ByteStore;
  tag?: string;
  key?: string;
  onProgress?: ProgressFn;
}

export interface EnsureResult {
  bytes: Uint8Array<ArrayBuffer>;
  from: "store" | "download";
}

export async function ensureCached(spec: DownloadSpec, opts: EnsureOptions): Promise<EnsureResult> {
  const { store, tag, key, onProgress } = opts;

  if (store && tag && key) {
    try {
      const cached = await store.get(tag, key);
      if (cached) {
        if (spec.sizeBytes !== undefined || spec.sha256 !== undefined) {
          await verifyBytes(cached, spec);
        }
        return { bytes: cached, from: "store" };
      }
    } catch {
    }
  }

  const bytes = await downloadBytes(spec, onProgress);
  if (store && tag && key) {
    try {
      await store.put(tag, key, bytes);
    } catch {
    }
  }
  return { bytes, from: "download" };
}
