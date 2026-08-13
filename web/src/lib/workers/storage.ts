import { idbGet, idbPut, openIdb } from "./idb";

export { openIdb };

export interface ByteStore {
  get(tag: string, key: string): Promise<Uint8Array<ArrayBuffer> | null>;
  put(tag: string, key: string, bytes: Uint8Array<ArrayBuffer>): Promise<boolean>;
}

export function hasOpfs(): boolean {
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- capability probe; `navigator` is undeclared in some runtimes (Node test env), so a bare reference would throw before the property check
  return typeof navigator !== "undefined" && !!navigator.storage?.getDirectory;
}

export function createOpfsStore(rootDir: string): ByteStore {
  if (!hasOpfs()) throw new Error("OPFS is not available in this environment");

  async function tagDir(tag: string, create: boolean): Promise<FileSystemDirectoryHandle> {
    const root = await navigator.storage.getDirectory();
    const top = await root.getDirectoryHandle(rootDir, { create });
    return top.getDirectoryHandle(tag, { create });
  }

  return {
    async get(tag, key) {
      try {
        const dir = await tagDir(tag, false);
        const fh = await dir.getFileHandle(key);
        const file = await fh.getFile();
        return new Uint8Array(await file.arrayBuffer());
      } catch (err) {
        if (err instanceof DOMException && err.name === "NotFoundError") return null;
        throw err;
      }
    },
    async put(tag, key, bytes) {
      const dir = await tagDir(tag, true);
      const fh = await dir.getFileHandle(key, { create: true });
      const writable = await fh.createWritable();
      try {
        await writable.write(bytes);
      } finally {
        await writable.close();
      }
      return true;
    },
  };
}

export function createIdbStore(dbName: string, storeName: string): ByteStore {
  const idbKey = (tag: string, key: string): string => `${tag}:${key}`;
  return {
    async get(tag, key) {
      try {
        const db = await openIdb(dbName, storeName);
        const buf = await idbGet<ArrayBuffer>(db, storeName, idbKey(tag, key));
        return buf instanceof ArrayBuffer ? new Uint8Array(buf) : null;
      } catch {
        return null;
      }
    },
    async put(tag, key, bytes) {
      try {
        const db = await openIdb(dbName, storeName);
        await idbPut(db, storeName, bytes.buffer, idbKey(tag, key));
        return true;
      } catch {
        return false;
      }
    },
  };
}
