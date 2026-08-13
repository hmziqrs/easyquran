import type { DownloadableSpec } from "$lib/data/quran-types";

import { downloadBytes, verifyBytes, type DownloadSpec, type ProgressFn } from "./download";
import { idbDelete, idbGet, runTxVoid } from "./idb";
import { idbError } from "./idb-error";
import { stampLastUsed } from "./opfs-retention";
import { createIdbStore, hasOpfs, openIdb } from "./storage";

export const ROOT_DIR = "easyquran";
export const QURAN_DB = "easyquran-quran";
export const QURAN_STORE = "artifacts";
export const POINTER_DB = "easyquran-pointers";
export const POINTER_STORE = "opfsPointers";

export const ACTIVE_SUFFIX = ".sqlite";
export const TEMP_SUFFIX = ".sqlite.tmp";
export const QURAN_ROW_COUNT = 6236;

export type CacheStore = "opfs" | "idb" | "session";
export interface CachedArtifact {
  bytes: Uint8Array<ArrayBuffer>;
  store: CacheStore;
  downloaded: boolean;
}
export interface CachedArtifactInfo {
  readonly id: string;
  readonly store: CacheStore;
  readonly tag: string;
  readonly sizeBytes: number;
}

export interface ActivePointer {
  readonly sourceId: string;
  readonly activeFile: string;
}

export type StagedValidator = (
  bytes: Uint8Array<ArrayBuffer>,
  spec: DownloadableSpec,
) => Promise<void> | void;

export class StagedValidationRejection extends Error {
  constructor(rejection: unknown) {
    const msg = rejection instanceof Error ? rejection.message : String(rejection);
    super(`staged validation rejected DB: ${msg}`);
    this.name = "StagedValidationRejection";
  }
}

async function runStagedValidator(
  validate: StagedValidator | undefined,
  bytes: Uint8Array<ArrayBuffer>,
  spec: DownloadableSpec,
): Promise<void> {
  if (!validate) return;
  try {
    await validate(bytes, spec);
  } catch (err) {
    throw new StagedValidationRejection(err);
  }
}

export function activeFileName(id: string): string {
  return `${id}${ACTIVE_SUFFIX}`;
}

export function tempFileName(id: string): string {
  return `${id}${TEMP_SUFFIX}`;
}

export function isTempFileName(name: string): boolean {
  return name.endsWith(TEMP_SUFFIX);
}

export function isActiveFileName(name: string): boolean {
  return name.endsWith(ACTIVE_SUFFIX) && !isTempFileName(name);
}

export function idFromActiveFileName(name: string): string | null {
  if (!isActiveFileName(name)) return null;
  return name.slice(0, -ACTIVE_SUFFIX.length);
}

export interface LegacyAdoptionInput {
  readonly pointer: ActivePointer | null;
  readonly candidateFile: string | null;
}
export interface LegacyAdoptionDecision {
  readonly adopt: boolean;
  readonly activeFile: string | null;
}
export function decideLegacyAdoption(input: LegacyAdoptionInput): LegacyAdoptionDecision {
  if (input.pointer) {
    return { adopt: false, activeFile: input.pointer.activeFile };
  }
  if (!input.candidateFile) return { adopt: false, activeFile: null };
  return { adopt: true, activeFile: input.candidateFile };
}

export interface PromotionInput {
  readonly oldPointer: ActivePointer | null;
  readonly newActiveFile: string;
  readonly validationOk: boolean;
}
export interface PromotionDecision {
  readonly commitPointer: boolean;
  readonly removeOldFile: boolean;
  readonly oldFile: string | null;
}
export function decidePromotion(input: PromotionInput): PromotionDecision {
  if (!input.validationOk) return { commitPointer: false, removeOldFile: false, oldFile: null };
  const oldFile = input.oldPointer?.activeFile ?? null;
  const removeOldFile = oldFile !== null && oldFile !== input.newActiveFile;
  return { commitPointer: true, removeOldFile, oldFile };
}

export interface SourceFilterInput {
  readonly pointers: ReadonlyMap<string, ActivePointer>;
  readonly files: ReadonlyArray<{ readonly tag: string; readonly name: string }>;
}
export interface SourceFilterResult {
  readonly sources: ReadonlyArray<{
    readonly id: string;
    readonly tag: string;
    readonly name: string;
  }>;
  readonly abandonedTemps: ReadonlyArray<{ readonly tag: string; readonly name: string }>;
}
export function filterSourceFiles(input: SourceFilterInput): SourceFilterResult {
  const sources: { id: string; tag: string; name: string }[] = [];
  const abandonedTemps: { tag: string; name: string }[] = [];
  for (const f of input.files) {
    if (isTempFileName(f.name)) {
      abandonedTemps.push({ tag: f.tag, name: f.name });
      continue;
    }
    if (f.tag !== f.name.slice(0, -ACTIVE_SUFFIX.length)) continue;
    const pointer = input.pointers.get(f.tag);
    if (!pointer || pointer.sourceId !== f.tag || pointer.activeFile !== f.name) continue;
    sources.push({ id: f.tag, tag: f.tag, name: f.name });
  }
  return { sources, abandonedTemps };
}

function toDownloadSpec(spec: DownloadableSpec): DownloadSpec {
  return {
    url: spec.downloadUrl,
    sizeBytes: spec.sizeBytes,
    label: spec.id,
  };
}

export async function readPointer(sourceId: string): Promise<ActivePointer | null> {
  try {
    const db = await openIdb(POINTER_DB, POINTER_STORE);
    const rec = await idbGet<ActivePointer>(db, POINTER_STORE, sourceId);
    if (
      rec &&
      typeof rec.sourceId === "string" &&
      typeof rec.activeFile === "string" &&
      rec.sourceId === sourceId
    ) {
      return { sourceId: rec.sourceId, activeFile: rec.activeFile };
    }
    return null;
  } catch {
    return null;
  }
}

export async function writePointer(pointer: ActivePointer): Promise<void> {
  await runTxVoid(await openIdb(POINTER_DB, POINTER_STORE), POINTER_STORE, "readwrite", (s) => {
    s.put({ sourceId: pointer.sourceId, activeFile: pointer.activeFile }, pointer.sourceId);
  });
}

export async function clearPointer(sourceId: string): Promise<void> {
  try {
    await idbDelete(await openIdb(POINTER_DB, POINTER_STORE), POINTER_STORE, sourceId);
  } catch {}
}

async function readAllPointers(): Promise<Map<string, ActivePointer>> {
  const out = new Map<string, ActivePointer>();
  try {
    const db = await openIdb(POINTER_DB, POINTER_STORE);
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(POINTER_STORE, "readonly");
      const req = tx.objectStore(POINTER_STORE).openCursor();
      req.onsuccess = () => {
        const cur = req.result;
        if (!cur) return;
        const v = cur.value as Partial<ActivePointer> | undefined;
        if (
          v &&
          typeof v.sourceId === "string" &&
          typeof v.activeFile === "string" &&
          typeof cur.key === "string"
        ) {
          out.set(cur.key, { sourceId: v.sourceId, activeFile: v.activeFile });
        }
        cur.continue();
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(idbError(tx.error, "transaction"));
    });
  } catch {}
  return out;
}

async function readOpfsFile(tag: string, name: string): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const root = await navigator.storage.getDirectory();
    const top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
    const dir = await top.getDirectoryHandle(tag, { create: false });
    const fh = await dir.getFileHandle(name);
    const file = await fh.getFile();
    return new Uint8Array(await file.arrayBuffer());
  } catch (err) {
    if (err instanceof DOMException && err.name === "NotFoundError") return null;
    throw err;
  }
}

async function writeOpfsFile(
  tag: string,
  name: string,
  bytes: Uint8Array<ArrayBuffer>,
): Promise<void> {
  const root = await navigator.storage.getDirectory();
  const top = await root.getDirectoryHandle(ROOT_DIR, { create: true });
  const dir = await top.getDirectoryHandle(tag, { create: true });
  const fh = await dir.getFileHandle(name, { create: true });
  const writable = await fh.createWritable();
  try {
    await writable.write(bytes);
  } finally {
    await writable.close();
  }
}

async function moveOpfsFile(tag: string, fromName: string, toName: string): Promise<void> {
  const root = await navigator.storage.getDirectory();
  const top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
  const dir = await top.getDirectoryHandle(tag, { create: false });
  const fh = await dir.getFileHandle(fromName);
  const movable = fh as FileSystemFileHandle & {
    move?: (name: string) => Promise<FileSystemFileHandle>;
  };
  if (typeof movable.move === "function") {
    await movable.move(toName);
    return;
  }
  const file = await fh.getFile();
  const bytes = new Uint8Array(await file.arrayBuffer());
  await writeOpfsFile(tag, toName, bytes);
  await dir.removeEntry(fromName);
}

async function removeOpfsFile(tag: string, name: string): Promise<void> {
  try {
    const root = await navigator.storage.getDirectory();
    const top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
    const dir = await top.getDirectoryHandle(tag, { create: false });
    await dir.removeEntry(name);
  } catch {}
}

async function listOpfsTree(): Promise<{ tag: string; name: string }[]> {
  const out: { tag: string; name: string }[] = [];
  if (!hasOpfs()) return out;
  let top: FileSystemDirectoryHandle;
  try {
    const root = await navigator.storage.getDirectory();
    top = await root.getDirectoryHandle(ROOT_DIR, { create: false });
  } catch {
    return out;
  }
  try {
    for await (const tagName of top.keys()) {
      let tagDir: FileSystemDirectoryHandle;
      try {
        tagDir = await top.getDirectoryHandle(tagName, { create: false });
      } catch {
        continue;
      }
      for await (const fileName of tagDir.keys()) {
        out.push({ tag: tagName, name: fileName });
      }
    }
  } catch (err) {
    console.warn("[opfs-cache] OPFS tree listing failed:", err);
  }
  return out;
}

export async function sweepAbandonedTemps(): Promise<void> {
  const files = await listOpfsTree();
  await Promise.all(
    files.filter((f) => isTempFileName(f.name)).map((f) => removeOpfsFile(f.tag, f.name)),
  );
}

export interface EnsureArtifactOptions {
  readonly validate?: StagedValidator;
}

export async function ensureArtifact(
  spec: DownloadableSpec,
  onProgress?: (loaded: number, total: number) => void,
  options: EnsureArtifactOptions = {},
): Promise<CachedArtifact> {
  const dl = toDownloadSpec(spec);
  const progress: ProgressFn | undefined = onProgress
    ? (p) => onProgress(p.loaded, p.total)
    : undefined;

  if (hasOpfs()) {
    try {
      return await ensureOpfsArtifact(spec, dl, progress, options.validate);
    } catch (err) {
      if (err instanceof StagedValidationRejection) throw err;
      console.warn(`[opfs-cache] OPFS unavailable for ${spec.id}, falling back:`, err);
    }
  }

  return ensureIdbArtifact(spec, dl, progress, options.validate);
}

async function ensureOpfsArtifact(
  spec: DownloadableSpec,
  dl: DownloadSpec,
  progress: ProgressFn | undefined,
  validate: StagedValidator | undefined,
): Promise<CachedArtifact> {
  const active = activeFileName(spec.id);
  const pointer = await readPointer(spec.id);

  if (pointer) {
    const bytes = await readOpfsFile(spec.id, pointer.activeFile);
    if (bytes) {
      try {
        await verifyBytes(bytes, dl);
        void stampLastUsed(spec.id);
        return { bytes, store: "opfs", downloaded: false };
      } catch {}
    }
  } else {
    const legacy = await readOpfsFile(spec.id, active);
    const adoption = decideLegacyAdoption({
      pointer: null,
      candidateFile: legacy ? active : null,
    });
    if (adoption.adopt && legacy) {
      try {
        await verifyBytes(legacy, dl);
        await runStagedValidator(validate, legacy, spec);
        await writePointer({ sourceId: spec.id, activeFile: active });
        void stampLastUsed(spec.id);
        return { bytes: legacy, store: "opfs", downloaded: false };
      } catch (err) {
        console.warn(`[opfs-cache] legacy ${active} invalid, redownloading:`, err);
      }
    }
  }

  const temp = tempFileName(spec.id);
  let staged: Uint8Array<ArrayBuffer> | null = null;
  try {
    const bytes = await downloadBytes(dl, progress);
    await writeOpfsFile(spec.id, temp, bytes);
    staged = await readOpfsFile(spec.id, temp);
    if (!staged) throw new Error(`[opfs-cache] ${spec.id}: staged temp unreadable`);
    await runStagedValidator(validate, staged, spec);
  } catch (err) {
    await removeOpfsFile(spec.id, temp);
    throw err;
  }

  const decision = decidePromotion({
    oldPointer: pointer,
    newActiveFile: active,
    validationOk: true,
  });
  if (!decision.commitPointer) {
    await removeOpfsFile(spec.id, temp);
    throw new Error(`[opfs-cache] ${spec.id}: staged DB rejected`);
  }

  await moveOpfsFile(spec.id, temp, active);
  await writePointer({ sourceId: spec.id, activeFile: active });

  const reopened = await readOpfsFile(spec.id, active);
  if (!reopened) {
    throw new Error(`[opfs-cache] ${spec.id}: active file missing after switch`);
  }
  await verifyBytes(reopened, dl);

  if (decision.removeOldFile && decision.oldFile) {
    await removeOpfsFile(spec.id, decision.oldFile);
  }

  await stampLastUsed(spec.id);
  return { bytes: staged ?? reopened, store: "opfs", downloaded: true };
}

async function ensureIdbArtifact(
  spec: DownloadableSpec,
  dl: DownloadSpec,
  progress: ProgressFn | undefined,
  validate: StagedValidator | undefined,
): Promise<CachedArtifact> {
  const store = createIdbStore(QURAN_DB, QURAN_STORE);
  const cached = await store.get(spec.id, spec.id);
  if (cached) {
    try {
      await verifyBytes(cached, dl);
      void stampLastUsed(spec.id);
      return { bytes: cached, store: "idb", downloaded: false };
    } catch {}
  }
  const bytes = await downloadBytes(dl, progress);
  await runStagedValidator(validate, bytes, spec);
  await store.put(spec.id, spec.id, bytes);
  await stampLastUsed(spec.id);
  return { bytes, store: "idb", downloaded: true };
}

async function listOpfsArtifacts(): Promise<CachedArtifactInfo[]> {
  const out: CachedArtifactInfo[] = [];
  if (!hasOpfs()) return out;
  const [files, pointers] = await Promise.all([listOpfsTree(), readAllPointers()]);
  const { sources, abandonedTemps } = filterSourceFiles({ pointers, files });
  for (const s of sources) {
    let sizeBytes = 0;
    try {
      const fh = await (
        await navigator.storage.getDirectory()
      )
        .getDirectoryHandle(ROOT_DIR, { create: false })
        .then((top) => top.getDirectoryHandle(s.tag, { create: false }))
        .then((dir) => dir.getFileHandle(s.name));
      sizeBytes = (await fh.getFile()).size;
    } catch {}
    out.push({ id: s.id, store: "opfs", tag: s.tag, sizeBytes });
  }
  if (abandonedTemps.length > 0) {
    void Promise.all(abandonedTemps.map((f) => removeOpfsFile(f.tag, f.name))).catch(() => {});
  }
  return out;
}

async function listIdbArtifacts(): Promise<CachedArtifactInfo[]> {
  const out: CachedArtifactInfo[] = [];
  try {
    const db = await openIdb(QURAN_DB, QURAN_STORE);
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(QURAN_STORE, "readonly");
      const req = tx.objectStore(QURAN_STORE).openCursor();
      req.onsuccess = () => {
        const cur = req.result;
        if (!cur) return;
        const compound = typeof cur.key === "string" ? cur.key : "";
        const sep = compound.indexOf(":");
        if (sep > 0) {
          const tag = compound.slice(0, sep);
          const id = compound.slice(sep + 1);
          const v = cur.value;
          const sizeBytes = v instanceof ArrayBuffer ? v.byteLength : 0;
          out.push({ id, store: "idb", tag, sizeBytes });
        }
        cur.continue();
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(idbError(tx.error, "transaction"));
    });
  } catch (err) {
    console.warn("[opfs-cache] IDB listing failed:", err);
  }
  return out;
}

export async function listCachedArtifacts(): Promise<CachedArtifactInfo[]> {
  const [opfs, idb] = await Promise.all([listOpfsArtifacts(), listIdbArtifacts()]);
  const byId = new Map<string, CachedArtifactInfo>();
  for (const info of idb) byId.set(info.id, info);
  for (const info of opfs) byId.set(info.id, info);
  return [...byId.values()];
}

export async function deleteCachedArtifact(id: string, tag: string): Promise<void> {
  if (hasOpfs()) {
    await removeOpfsFile(tag, activeFileName(id));
    await removeOpfsFile(tag, tempFileName(id));
  }
  await clearPointer(id);
  try {
    await runTxVoid(await openIdb(QURAN_DB, QURAN_STORE), QURAN_STORE, "readwrite", (s) => {
      s.delete(`${tag}:${id}`);
    });
  } catch {}
}
