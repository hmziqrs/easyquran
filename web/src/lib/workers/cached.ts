import { downloadBytes, verifyBytes } from "./download";
import type { DownloadSpec, ProgressFn } from "./download";
import type { ByteStore } from "./storage";

export interface EnsureOptions {
  store?: ByteStore;
  version?: string;
  key?: string;
  onProgress?: ProgressFn;
}

export interface EnsureResult {
  bytes: Uint8Array<ArrayBuffer>;
  from: "store" | "download";
}

export async function ensureCached(spec: DownloadSpec, opts: EnsureOptions): Promise<EnsureResult> {
  const { store, version, key, onProgress } = opts;

  if (store && version && key) {
    try {
      const cached = await store.get(version, key);
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
  if (store && version && key) {
    try {
      await store.put(version, key, bytes);
    } catch {
    }
  }
  return { bytes, from: "download" };
}
